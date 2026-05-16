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
use std::rc::Rc;

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    /// Matches first or last line of a drawer
    /// Group 1 contains drawer's name or "END"
    /// Note: (?m) enables multiline mode so ^ matches line start
    pub static ref REGEX_DRAWER: Regex = Regex::new(r"(?im)^[ \t]*:((?:\w|[-_])+):[ \t]*$").unwrap();
}

#[derive(Debug)]
pub struct DrawerData<'a> {
    /// Drawer's name (string).
    pub drawer_name: &'a str,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Parse a drawer element.
    ///
    /// Format: `:NAME:\n...content...\n:END:`
    /// Case insensitive (matches :NAME: and :END:)
    pub fn drawer_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let input_slice = &self.input[start..limit];

        // Check if we start with a drawer begin line
        if let Some(caps) = REGEX_DRAWER.captures(input_slice) {
            let name = caps.get(1).map_or("", |m| m.as_str());

            // If it's "END", not a start
            if name.eq_ignore_ascii_case("END") {
                return SyntaxNode::fallback(self.input, start, limit);
            }

            // Find the :END: line
            let mut search_pos = start;
            let mut end = limit;
            let mut found_end = false;

            while search_pos < limit {
                if let Some(cap) = REGEX_DRAWER.captures(&self.input[search_pos..limit]) {
                    if let Some(m) = cap.get(1) {
                        if m.as_str().eq_ignore_ascii_case("END") {
                            // Found :END:, calculate end position
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
                            break;
                        }
                    }
                }
                // Move to next line
                if let Some(nl) = self.input[search_pos..limit].find('\n') {
                    search_pos += nl + 1;
                } else {
                    break;
                }
            }

            if !found_end {
                return SyntaxNode::fallback(self.input, start, limit);
            }

            let post_blank = if end < limit {
                let remaining = &self.input[end..limit];
                let trimmed = remaining.trim_start();
                (remaining.len() - trimmed.len()).min(2)
            } else {
                0
            };

            let children = if name.eq_ignore_ascii_case("PROPERTIES") {
                self.parse_property_drawer_contents(start, end)
            } else {
                vec![]
            };

            return SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(children),
                data: if name.eq_ignore_ascii_case("PROPERTIES") {
                    Syntax::PropertyDrawer
                } else {
                    Syntax::Drawer(Box::new(DrawerData { drawer_name: name }))
                },
                location: Interval { start, end },
                content_location: None,
                post_blank,
                affiliated,
            };
        }

        SyntaxNode::fallback(self.input, start, limit)
    }

    fn parse_property_drawer_contents(&self, start: usize, end: usize) -> Vec<Rc<SyntaxNode<'a>>> {
        let mut children = Vec::new();
        let input = self.input;

        let first_line_end = match input[start..].find('\n') {
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
            let line_end = match content[pos..].find('\n') {
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
                children.push(Rc::new(SyntaxNode {
                    parent: RefCell::new(None),
                    children: RefCell::new(vec![]),
                    data: Syntax::NodeProperty(Box::new(node_data)),
                    location: Interval {
                        start: prop_start,
                        end: prop_end,
                    },
                    content_location: None,
                    post_blank: 0,
                    affiliated: None,
                }));
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
