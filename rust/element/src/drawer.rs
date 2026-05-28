//    This file is part of org-rs.
//
//    org-rs is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    org-rs is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with org-rs.  If not, see <https://www.gnu.org/licenses/>.

use crate::affiliated::ElementSpan;
use crate::cursor::CachedRegex;
use crate::data::{BumpVec, Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use lazy_static::lazy_static;
use memchr::{memchr, memmem, memrchr};
use regex::Regex;

lazy_static! {
    /// Matches first or last line of a drawer.
    /// Group 1 contains the drawer's name or `END`.
    pub static ref REGEX_DRAWER: CachedRegex =
        CachedRegex::new(Regex::new(r"(?im)^[ \t]*:((?:\w|[-_])+):[ \t]*$").unwrap());
}

/// Locate the byte position past the `:END:` line within `bytes[start..limit]`
/// using a single SIMD substring scan, verifying that only ASCII whitespace
/// surrounds the marker on its line.
///
/// Returns `Some(end)` where `end` is the position of the first byte of the
/// line following `:END:`, or `limit` if `:END:` ends at the region boundary.
/// Returns `None` if no valid uppercase `:END:` line is found.
#[inline]
fn find_end_memmem(bytes: &[u8], start: usize, limit: usize) -> Option<usize> {
    for off in memmem::find_iter(&bytes[start..limit], b":END:") {
        let pos = start + off;
        let line_start = memrchr(b'\n', &bytes[start..pos]).map_or(start, |i| start + i + 1);
        if !bytes[line_start..pos]
            .iter()
            .all(|&b| b == b' ' || b == b'\t')
        {
            continue;
        }
        let after = pos + 5;
        let line_end = memchr(b'\n', &bytes[after..limit]).map_or(limit, |i| after + i);
        if !bytes[after..line_end]
            .iter()
            .all(|&b| b == b' ' || b == b'\t')
        {
            continue;
        }
        return Some(if line_end < limit {
            line_end + 1
        } else {
            line_end
        });
    }
    None
}

/// Fallback `:END:` search used only when the SIMD fast path found no uppercase
/// match.  Walks line-by-line with a case-insensitive comparison to handle
/// exotic casing such as `:end:` or `:End:`.
#[cold]
fn find_end_linewise(input: &str, start: usize, limit: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut pos = start;
    while pos < limit {
        let line_end = memchr(b'\n', &bytes[pos..limit]).map_or(limit, |i| pos + i);
        let line = &input[pos..line_end];
        if is_end_line(line) {
            return Some(if line_end < limit {
                line_end + 1
            } else {
                line_end
            });
        }
        pos = if line_end < limit {
            line_end + 1
        } else {
            limit
        };
    }
    None
}

/// Returns `true` when `line` (a single line without its trailing newline) is
/// a drawer-end marker: `:END:` with optional surrounding ASCII whitespace,
/// case-insensitive.
#[inline]
fn is_end_line(line: &str) -> bool {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    bytes.len() >= 5
        && bytes[0] == b':'
        && bytes[bytes.len() - 1] == b':'
        && bytes[1..bytes.len() - 1].eq_ignore_ascii_case(b"END")
}

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    /// Parse a drawer element.
    ///
    /// Format: `:NAME:\n...content...\n:END:`
    /// Case insensitive (matches :NAME: and :END:)
    #[inline]
    pub fn drawer_parser(&mut self, element_span: ElementSpan<'a, 'b>) -> NodeId {
        let ElementSpan {
            span: Interval { start, end: limit },
            affiliated,
        } = element_span;
        let input_slice = &self.input[start..limit];

        if let Some(caps) = REGEX_DRAWER.captures(input_slice) {
            let name = caps.get(1).map_or("", |m| m.as_str());

            if name.eq_ignore_ascii_case("END") {
                return self
                    .arena
                    .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump));
            }

            let bytes = self.input.as_bytes();
            let end_pos = find_end_memmem(bytes, start, limit)
                .or_else(|| find_end_linewise(self.input, start, limit));

            let (end, found_end) = match end_pos {
                Some(e) => (e, true),
                None => (limit, false),
            };

            if !found_end {
                return self
                    .arena
                    .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump));
            }

            let post_blank = if end < limit {
                let remaining = &self.input[end..limit];
                remaining.len() - remaining.trim_start().len()
            } else {
                0
            }
            .min(2);

            let data = if name.eq_ignore_ascii_case("PROPERTIES") {
                Syntax::PropertyDrawer
            } else {
                Syntax::Drawer(name)
            };

            let node = self.arena.alloc(
                SyntaxNode::new(data, (start, end), self.bump)
                    .post_blank(post_blank)
                    .affiliated(affiliated)
                    .build(),
            );

            if name.eq_ignore_ascii_case("PROPERTIES") {
                let children = self.parse_property_drawer_contents(start, end);
                self.arena.set_children(node, children);
            }

            return node;
        }

        self.arena
            .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump))
    }

    fn parse_property_drawer_contents(&mut self, start: usize, end: usize) -> BumpVec<'b, NodeId> {
        let mut children = BumpVec::new_in(self.bump);
        let input = self.input;

        let first_line_end = match memchr(b'\n', &input.as_bytes()[start..]) {
            Some(nl) => start + nl + 1,
            None => return children,
        };

        // `end` points past the `:END:\n` line.  Strip back to the newline that
        // precedes `:END:` so the content slice never includes the end marker.
        let content_end = {
            let before_end_nl = if end > 0 && input.as_bytes().get(end - 1) == Some(&b'\n') {
                end - 1
            } else {
                end
            };
            if before_end_nl > first_line_end {
                memrchr(b'\n', &input.as_bytes()[first_line_end..before_end_nl])
                    .map(|i| first_line_end + i + 1)
                    .unwrap_or(first_line_end)
            } else {
                first_line_end
            }
        };

        if first_line_end >= content_end {
            return children;
        }

        let content = &input[first_line_end..content_end];
        let mut pos = 0;

        while pos < content.len() {
            let line_end = match memchr(b'\n', &content.as_bytes()[pos..]) {
                Some(nl) => pos + nl,
                None => content.len(),
            };
            let line = &content[pos..line_end];

            if let Some((key, value)) = Self::parse_node_property_line(line) {
                let prop_start = first_line_end + pos;
                let prop_end = if line_end < content.len() {
                    line_end + 1
                } else {
                    line_end
                };
                let node_data = crate::headline::NodePropertyData { key, value };
                children.push(
                    self.arena.alloc(
                        SyntaxNode::new(
                            Syntax::NodeProperty(self.bump.alloc(node_data)),
                            (prop_start, prop_end),
                            self.bump,
                        )
                        .build(),
                    ),
                );
            }

            pos = if line_end < content.len() {
                line_end + 1
            } else {
                content.len()
            };
        }

        children
    }

    fn parse_node_property_line(line: &str) -> Option<(&str, &str)> {
        let trimmed = line.trim();
        if !trimmed.starts_with(':') {
            return None;
        }

        let rest = &trimmed[1..];
        if rest.is_empty() || rest.starts_with(':') {
            return None;
        }

        if let Some(colon_pos) = rest.find(':') {
            let key = &rest[..colon_pos];
            let value = rest[colon_pos + 1..].trim();
            if !key.is_empty() {
                return Some((key, value));
            }
        }

        None
    }
}
