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
use crate::data::{Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use memchr::memchr;

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    /// Parse a paragraph starting at `start`, bounded by `limit`.
    ///
    /// A paragraph extends from the current position to the first blank
    /// line or to the start of the next element-level construct
    /// (headline, keyword, block, etc.), whichever comes first.
    #[inline]
    pub fn paragraph_parser(&mut self, element_span: ElementSpan<'a, 'b>) -> NodeId {
        let ElementSpan {
            span: Interval { start, end: limit },
            affiliated,
            content_start,
        } = element_span;
        // Start the line scan at content_start (past any collected affiliated
        // keywords) so that `#+`-prefixed affiliated-keyword lines that were
        // already consumed by collect_affiliated_keywords are NOT mistaken for
        // element-start patterns.  The paragraph's outer span still covers
        // from `start` so affiliated keywords are included in the node's range.
        let mut end = content_start;
        let mut post_blank: usize = 0;

        // Advance line by line until we hit a blank line, an element
        // start pattern, or the limit.
        while end < limit {
            // Find end of current line.
            let line_end =
                memchr(b'\n', &self.input.as_bytes()[end..limit]).map_or(limit, |i| end + i + 1);

            // Inspect the current line (which includes the trailing newline).
            let line = &self.input[end..line_end];
            let bytes = line.as_bytes();

            // Blank line detection: no content bytes before the trailing `\n`.
            let line_len = if bytes.last() == Some(&b'\n') {
                bytes.len() - 1
            } else {
                bytes.len()
            };
            let content_offset = bytes[..line_len]
                .iter()
                .position(|&b| b != b' ' && b != b'\t');

            // Blank line — terminate the paragraph.
            let Some(i) = content_offset else {
                post_blank = 1;
                end = line_end;
                break;
            };

            // Check if this line starts a new element (only on the
            // second line onwards — the first line is always part of
            // this paragraph).
            if end > start {
                match bytes[i] {
                    b'*' => {
                        // Headline at column 0: all consecutive `*` followed by a space
                        if i == 0 {
                            let mut j = i + 1;
                            while bytes.get(j).is_some_and(|&b| b == b'*') {
                                j += 1;
                            }
                            if bytes.get(j).is_some_and(|&b| b == b' ') {
                                break;
                            }
                        }
                        // List item at any column (starts_with_item handles indent logic)
                        if i > 0 && crate::list::starts_with_item(&line[..line_len]) {
                            break;
                        }
                    }
                    // Keyword / block / comment
                    //
                    // In strict compliance mode we are more permissive
                    // about what stays inside a paragraph:
                    //
                    // - `#+begin_*`               → break (block starts)
                    // - `#+end_*`                 → stay (orphaned end
                    //   line in a broken block)
                    // - Anything else `#+...`     → break (let element
                    //   parser collect affiliated
                    //   keywords or create keyword
                    //   elements)
                    //
                    // Outside compliance mode we break at every `#+` as
                    // a sensible optimisation.
                    b'#' => {
                        if i + 1 < bytes.len() && bytes[i + 1] == b'+' {
                            if self.environment.emacs_compliance() {
                                let rest = &bytes[i + 2..line_len];
                                // `#+begin_*` → break (always)
                                if rest.len() > 4 && rest[..5].eq_ignore_ascii_case(b"begin") {
                                    break;
                                }
                                // `#+end_*` → stay in paragraph (orphaned end line)
                                if rest.len() > 2 && rest[..3].eq_ignore_ascii_case(b"end") {
                                    // stay in paragraph
                                } else {
                                    // Everything else → break so the element
                                    // parser can collect affiliated keywords
                                    // or create keyword / block elements.
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        if bytes.get(i + 1).is_none_or(|&b| b == b' ') {
                            break;
                        }
                    }
                    // List item or horizontal rule
                    b'-' | b'+' => {
                        match bytes.get(i + 1) {
                            // Bullet followed by space / tab / EOL → list item
                            Some(b' ' | b'\t' | b'\n') | None => break,
                            // Could be a horizontal rule if 5+ hyphens
                            _ if crate::markup::is_horizontal_rule(&line[i..line_len]) => break,
                            _ => {}
                        }
                    }
                    // Numbered list item
                    _ if bytes[i].is_ascii_digit()
                        && crate::list::starts_with_item(&line[..line_len]) =>
                    {
                        break
                    }
                    // Fixed-width line or drawer start. Only these actually
                    // begin a new element; a line like `:one: text` is neither
                    // (text follows the closing colon), so it stays part of the
                    // paragraph rather than splitting it.
                    b':' => {
                        let is_fixed_width = matches!(bytes.get(i + 1), None | Some(b' ' | b'\n'));
                        if is_fixed_width
                            || crate::drawer::is_drawer_header_line(&line[i..line_len])
                        {
                            break;
                        }
                    }
                    // Table row
                    b'|' => break,
                    _ => {}
                }
            }

            end = line_end;
        }

        // Ensure we advance at least one line to avoid infinite loops.
        if end == start {
            end = memchr(b'\n', &self.input.as_bytes()[start..limit])
                .map_or(limit, |i| start + i + 1);
        }

        let content_end = end - post_blank;
        let children = self.parse_objects((content_start, content_end), |_| true);

        self.arena.alloc_with_children(
            SyntaxNode::new(Syntax::Paragraph, (start, end), self.bump)
                .content((content_start, content_end))
                .post_blank(post_blank)
                .affiliated(affiliated)
                .build(),
            children,
        )
    }

    /// Parse a section bounded by `limit`.
    ///
    /// A section is a container that spans from the current cursor position
    /// to `limit`.  It is the body content under a headline (or the text
    /// before the first headline in the document).
    #[inline]
    pub fn section_parser(&mut self, limit: usize) -> NodeId {
        let begin = self.cursor.pos();
        self.arena.alloc(
            SyntaxNode::new(Syntax::Section, (begin, limit), self.bump)
                .content((begin, limit))
                .build(),
        )
    }
}
