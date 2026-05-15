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
//

use std::cell::RefCell;

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};

fn strip_fixed_width_colons(input: &str) -> &str {
    let mut result = input;
    while let Some(rest) = result.strip_prefix(": ") {
        result = rest;
    }
    while let Some(rest) = result.strip_prefix(":") {
        if rest.is_empty() || rest.starts_with('\n') {
            break;
        }
        result = rest;
    }
    result
}
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_HORIZONTAL_RULE: Regex = Regex::new(r"[ \t]*-{5,}[ \t]*$").unwrap();

    /// Regular expression matching the definition of a footnote.
    /// Match group 1 contains definition's label
    pub static ref REGEX_FOOTNOTE_DEFINITION: Regex = Regex::new(r"^\[fn:([-_[:word:]]+)\]").unwrap();


    /// Fixed Width Areas
    /// A “fixed-width line” start with a colon character and a whitespace or an end of line.
    /// Fixed width areas can contain any number of consecutive fixed-width lines.
    pub static ref REGEX_FIXED_WIDTH: Regex = Regex::new(r"[ \t]*:( |$)").unwrap();

}

#[derive(Debug)]
pub struct CommentData<'a> {
    /// Comments, with pound signs (string).
    pub value: &'a str,
}

#[derive(Debug)]
pub struct FixedWidthData<'a> {
    /// Contents, without colons prefix (string).
    pub value: &'a str,
}

/// Greater element
#[derive(Debug)]
pub struct FootnoteDefinitionData<'a> {
    /// Label used for references (string).
    pub label: &'a str,

    /// Number of newline characters between the
    /// beginning of the footnote and the beginning
    /// of the contents (0, 1 or 2).
    pub pre_blank: u8,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Parse a comment element starting at `start`.
    ///
    /// A comment is one or more consecutive lines starting with `# `
    /// (hash + space) or just `#` at end of line.
    pub fn comment_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let mut end = start;

        // Consume consecutive comment lines.
        while end < limit {
            let line_end = self.input[end..limit]
                .find('\n')
                .map_or(limit, |i| end + i + 1);
            let line = self.input[end..line_end].trim_start();
            if line.starts_with("# ") || line == "#" || line == "#\n" {
                end = line_end;
            } else {
                break;
            }
        }

        if end == start {
            end = self.input[start..limit]
                .find('\n')
                .map_or(limit, |i| start + i + 1);
        }

        let value = &self.input[start..end];

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Comment(Box::new(CommentData { value })),
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Parse a horizontal rule at `start`.
    ///
    /// A horizontal rule is a line containing at least five consecutive
    /// dashes and nothing else (ignoring surrounding whitespace).
    pub fn horizontal_rule_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i + 1);

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::HorizontalRule,
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Fallback: footnote definition parser (not yet fully implemented).
    pub fn footnote_definition_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        SyntaxNode::fallback(self.input, start, limit)
    }

    /// Parse a fixed-width element.
    ///
    /// A fixed-width area consists of consecutive lines starting with
    /// a colon and whitespace (or just colon at end of line).
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound the search
    /// - Uses `aff_start` for begin position  
    /// - Uses affiliated data from parameter
    /// - Value is stored WITHOUT colons (stripped at parse time)
    /// - Calculates post-blank
    pub fn fixed_width_parser(
        &self,
        limit: usize,
        aff_start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let begin = aff_start;
        let mut pos_before_blank = self.cursor.borrow().pos();
        let mut end_area = pos_before_blank;

        while end_area < limit {
            let line_end = self.input[end_area..limit]
                .find('\n')
                .map_or(limit, |i| end_area + i + 1);

            let line = &self.input[end_area..line_end];
            let trimmed = line.trim_start();

            if trimmed.is_empty() {
                break;
            }

            if !trimmed.starts_with(':') {
                break;
            }

            end_area = line_end;
        }

        if end_area == pos_before_blank {
            end_area = self.input[pos_before_blank..limit]
                .find('\n')
                .map_or(limit, |i| pos_before_blank + i + 1);
        }

        pos_before_blank = end_area;

        let mut end = end_area;
        while end < limit {
            let c = self.input.as_bytes().get(end).copied();
            match c {
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => end += 1,
                _ => break,
            }
        }
        if end >= self.input.len() {
            end = self.input.len();
        } else {
            while end > 0 {
                let c = self.input.as_bytes().get(end - 1).copied();
                match c {
                    Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => end -= 1,
                    _ => break,
                }
            }
        }

        let raw_value = &self.input[pos_before_blank..end];
        let value = strip_fixed_width_colons(raw_value);

        let mut post_blank = 0;
        let mut check_pos = end;
        while check_pos < limit {
            if self.input.as_bytes().get(check_pos).copied() == Some(b'\n') {
                post_blank += 1;
                check_pos += 1;
                if check_pos < limit && self.input.as_bytes().get(check_pos).copied() == Some(b'\n')
                {
                    break;
                }
            } else if self.input.as_bytes().get(check_pos).copied() == Some(b' ')
                || self.input.as_bytes().get(check_pos).copied() == Some(b'\t')
            {
                post_blank += 1;
                check_pos += 1;
            } else {
                break;
            }
        }

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::FixedWidth(Box::new(FixedWidthData { value })),
            location: Interval { start: begin, end },
            content_location: None,
            post_blank,
            affiliated,
        }
    }
}
