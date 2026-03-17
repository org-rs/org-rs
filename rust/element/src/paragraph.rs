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

use std::cell::RefCell;

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Parse a paragraph starting at `start`, bounded by `limit`.
    ///
    /// A paragraph extends from the current position to the first blank
    /// line or to the start of the next element-level construct
    /// (headline, keyword, block, etc.), whichever comes first.
    pub fn paragraph_parser(
        &self,
        limit: usize,
        start: usize,
        _maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let bytes = self.input.as_bytes();
        let mut end = start;

        // Advance line by line until we hit a blank line, an element
        // start pattern, or the limit.
        while end < limit {
            // Find end of current line.
            let line_end = self.input[end..limit]
                .find('\n')
                .map_or(limit, |i| end + i + 1);

            // Check if this line is blank (only whitespace).
            let line = &self.input[end..line_end];
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Include the blank line in post_blank but stop content here.
                end = line_end;
                break;
            }

            // Check if this line starts a new element (only on the
            // second line onwards — the first line is always part of
            // this paragraph).
            if end > start {
                if trimmed.starts_with('*') && trimmed.len() > 1
                    && trimmed.as_bytes().get(1).map_or(false, |&b| b == b' ' || b == b'*')
                {
                    break; // headline
                }
                if trimmed.starts_with("#+") {
                    break; // keyword or block
                }
                if trimmed.starts_with("# ") || trimmed == "#" {
                    break; // comment
                }
                if trimmed.starts_with("-----") {
                    break; // horizontal rule
                }
            }

            end = line_end;
        }

        // Ensure we advance at least one line to avoid infinite loops.
        if end == start {
            end = self.input[start..limit]
                .find('\n')
                .map_or(limit, |i| start + i + 1);
        }

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Paragraph,
            location: Interval { start, end },
            content_location: Some(Interval { start, end }),
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Parse a section bounded by `limit`.
    ///
    /// A section is a container that spans from the current cursor position
    /// to `limit`.  It is the body content under a headline (or the text
    /// before the first headline in the document).
    pub fn section_parser(&self, limit: usize) -> SyntaxNode<'a> {
        let begin = self.cursor.borrow().pos();

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Section,
            location: Interval {
                start: begin,
                end: limit,
            },
            content_location: Some(Interval {
                start: begin,
                end: limit,
            }),
            post_blank: 0,
            affiliated: None,
        }
    }
}
