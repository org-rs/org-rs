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
use std::convert::TryInto;

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

    /// Diary Sexp elements - must be at beginning of line (unindented)
    /// Match group 1 contains the content after %%(
    /// Note: No ^ anchor needed - parser ensures we're at the right position
    pub static ref REGEX_DIARY_SEXP: Regex = Regex::new(r"%%\((.*)").unwrap();

    /// Fixed Width Areas
    /// A "fixed-width line" start with a colon character and a whitespace or an end of line.
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

    /// Raw text content (without the footnote marker).
    /// TODO: Add child element parsing when implementing element-level parsing.
    /// Currently stores raw text; caller can parse as needed.
    pub value: &'a str,
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

    /// Parse a footnote definition element.
    ///
    /// Footnote definition format: `[fn:LABEL] CONTENTS`
    /// - LABEL is digits or word characters (hyphens, underscores)
    /// - CONTENTS ends at: next footnote def, headline, 2 consecutive blanks, or buffer end
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound search
    /// - Uses `aff_start` for begin position
    /// - Stores raw text in value field
    /// - Sets content_location for contents_begin/contents_end
    /// - Calculates pre_blank and post_blank
    pub fn footnote_definition_parser(
        &self,
        limit: usize,
        aff_start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let begin = aff_start;
        let input_slice = &self.input[begin..limit];

        let label = match REGEX_FOOTNOTE_DEFINITION.captures(input_slice) {
            Some(caps) => caps.get(1).map(|m| m.as_str()).unwrap_or(""),
            None => return SyntaxNode::fallback(self.input, begin, limit),
        };

        let label_end = match REGEX_FOOTNOTE_DEFINITION.find(input_slice) {
            Some(m) => m.end(),
            None => return SyntaxNode::fallback(self.input, begin, limit),
        };

        let after_label = begin + label_end;
        let line_end_pos = self.input[after_label..limit]
            .find('\n')
            .map_or(limit, |i| after_label + i);

        let mut end = line_end_pos;
        let mut pre_blank: u8 = 0;

        let mut search_pos = line_end_pos;
        while search_pos < limit {
            let remaining = &self.input[search_pos..limit];

            if remaining.starts_with("[fn:") {
                break;
            }

            if remaining.starts_with('*') && remaining.chars().nth(1).map_or(false, |c| c == ' ') {
                break;
            }

            if remaining.starts_with("\n\n") {
                end = search_pos;
                pre_blank = 2;
                break;
            }

            if remaining.starts_with('\n') {
                let next_line = &remaining[1..];
                if next_line.trim().is_empty() {
                    end = search_pos;
                    pre_blank = 1;
                    break;
                }
            }

            if let Some(nl) = remaining.find('\n') {
                search_pos += nl + 1;
            } else {
                end = limit;
                break;
            }
        }

        if end == line_end_pos {
            end = limit;
            pre_blank = 0;
        }

        let contents_start_raw = after_label;
        let contents_start = self.input[contents_start_raw..]
            .find(|c: char| !c.is_whitespace())
            .map_or(contents_start_raw, |i| contents_start_raw + i);

        let contents_end = if pre_blank > 0 {
            let blank_start = if pre_blank == 2 {
                end
            } else {
                line_end_pos + 1
            };
            self.input[contents_start..blank_start].trim_end().len() + contents_start
        } else {
            self.input[contents_start..end].trim_end().len() + contents_start
        };

        let value = &self.input[contents_start..contents_end];

        let post_blank = if end < limit {
            let remaining = &self.input[end..limit];
            let trimmed = remaining.trim_start();
            ((remaining.len() - trimmed.len()) as u8).min(2) as usize
        } else {
            0
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::FootnoteDefinition(Box::new(FootnoteDefinitionData {
                label,
                pre_blank,
                value,
            })),
            location: Interval { start: begin, end },
            content_location: Some(Interval {
                start: contents_start,
                end: contents_end,
            }),
            post_blank: post_blank as usize,
            affiliated,
        }
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
