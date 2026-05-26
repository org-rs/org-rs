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
use crate::data::{Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use lazy_static::lazy_static;
use memchr::memchr;
use regex::Regex;

lazy_static! {
    /// Matches first or last line of a drawer
    /// Group 1 contains drawer's name or "END"
    /// Note: (?m) enables multiline mode so ^ matches line start
    pub static ref REGEX_DRAWER: CachedRegex =
        CachedRegex::new(Regex::new(r"(?im)^[ \t]*:((?:\w|[-_])+):[ \t]*$").unwrap());
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Parse a drawer element.
    ///
    /// Format: `:NAME:\n...content...\n:END:`
    /// Case insensitive (matches :NAME: and :END:)
    #[inline]
    pub fn drawer_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let input_slice = &self.input[start..limit];

        if let Some(caps) = REGEX_DRAWER.captures(input_slice) {
            let name = caps.get(1).map_or("", |m| m.as_str());

            if name.eq_ignore_ascii_case("END") {
                return self.arena.alloc(SyntaxNode::fallback(self.input, start, limit));
            }

            let mut search_pos = start;
            let mut end = limit;
            let mut found_end = false;

            while search_pos < limit {
                if let Some(cap) = REGEX_DRAWER.captures(&self.input[search_pos..limit]) {
                    if let Some(m) = cap.get(1) {
                        if m.as_str().eq_ignore_ascii_case("END") {
                            let match_start = search_pos + cap.get(0).map_or(0, |x| x.start());
                            let match_end = search_pos + cap.get(0).map_or(0, |x| x.end());
                            end = if match_end < limit
                                && self.input.as_bytes().get(match_end) == Some(&b'\n')
                            {
                                match_end + 1
                            } else {
                                match_end
                            };
                            found_end = true;
                            let _ = match_start;
                            break;
                        }
                    }
                }
                if let Some(nl) = memchr(b'\n', &self.input.as_bytes()[search_pos..limit]) {
                    search_pos += nl + 1;
                } else {
                    break;
                }
            }

            if !found_end {
                return self.arena.alloc(SyntaxNode::fallback(self.input, start, limit));
            }

            let post_blank = if end < limit {
                let remaining = &self.input[end..limit];
                remaining.len() - remaining.trim_start().len()
            } else {
                0
            }
            .min(2);

            let children = if name.eq_ignore_ascii_case("PROPERTIES") {
                self.parse_property_drawer_contents(start, end)
            } else {
                vec![]
            };

            let data = if name.eq_ignore_ascii_case("PROPERTIES") {
                Syntax::PropertyDrawer
            } else {
                Syntax::Drawer(name)
            };

            return self.arena.alloc_with_children(
                SyntaxNode::new(data, (start, end))
                    .post_blank(post_blank)
                    .affiliated(affiliated)
                    .build(),
                children,
            );
        }

        self.arena.alloc(SyntaxNode::fallback(self.input, start, limit))
    }

    fn parse_property_drawer_contents(&mut self, start: usize, end: usize) -> Vec<NodeId> {
        let mut children = Vec::new();
        let input = self.input;

        let first_line_end = match memchr(b'\n', &input.as_bytes()[start..]) {
            Some(nl) => start + nl + 1,
            None => return children,
        };

        let content_end = if end > 0 && input.as_bytes().get(end - 1) == Some(&b'\n') {
            end - 1
        } else {
            end
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
                let prop_end = if line_end < content.len() { line_end + 1 } else { line_end };
                let node_data = crate::headline::NodePropertyData { key, value };
                children.push(self.arena.alloc(
                    SyntaxNode::new(
                        Syntax::NodeProperty(Box::new(node_data)),
                        (prop_start, prop_end),
                    )
                    .build(),
                ));
            }

            pos = if line_end < content.len() { line_end + 1 } else { content.len() };
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
