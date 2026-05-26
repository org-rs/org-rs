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

use std::borrow::Cow;

use crate::affiliated::ElementSpan;
use crate::data::{Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use memchr::memchr;
use regex::Regex;

fn strip_line_prefix(line: &str) -> &str {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix(": ") {
        rest
    } else if t == ":" {
        ""
    } else {
        line
    }
}

/// Strip the `: ` prefix from each fixed-width line.
/// Returns a borrowed slice for single-line input (zero allocation),
/// or an owned String when multiple lines require reconstruction.
pub fn strip_fixed_width_colons(input: &str) -> Cow<'_, str> {
    let mut lines = input.lines();
    let first = match lines.next() {
        None => return Cow::Borrowed(""),
        Some(l) => l,
    };
    let second = lines.next();

    if second.is_none() {
        return Cow::Borrowed(strip_line_prefix(first));
    }

    let mut out = String::from(strip_line_prefix(first));
    out.push('\n');
    out.push_str(strip_line_prefix(second.unwrap()));
    for line in lines {
        out.push('\n');
        out.push_str(strip_line_prefix(line));
    }
    Cow::Owned(out)
}

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
    #[inline]
    pub fn comment_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let span = element_span.span;
        let mut end = span.start;

        while end < span.end {
            let line_end = memchr(b'\n', &self.input.as_bytes()[end..span.end])
                .map_or(span.end, |i| end + i + 1);
            let line = self.input[end..line_end].trim_start();
            if line.starts_with("# ") || line == "#" || line == "#\n" {
                end = line_end;
            } else {
                break;
            }
        }

        if end == span.start {
            end = memchr(b'\n', &self.input.as_bytes()[span.start..span.end])
                .map_or(span.end, |i| span.start + i + 1);
        }

        let value = &self.input[span.start..end];
        self.arena.alloc(SyntaxNode::new(Syntax::Comment(value), (span.start, end)).build())
    }

    /// Parse a horizontal rule at `start`.
    ///
    /// A horizontal rule is a line containing at least five consecutive
    /// dashes and nothing else (ignoring surrounding whitespace).
    #[inline]
    pub fn horizontal_rule_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let span = element_span.span;
        let end = memchr(b'\n', &self.input.as_bytes()[span.start..span.end]).map_or(span.end, |i| span.start + i + 1);
        self.arena.alloc(SyntaxNode::new(Syntax::HorizontalRule, (span.start, end)).build())
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
    #[inline]
    pub fn footnote_definition_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start: begin, end: limit }, affiliated } = element_span;
        let input_slice = &self.input[begin..limit];

        let label = match REGEX_FOOTNOTE_DEFINITION.captures(input_slice) {
            Some(caps) => caps.get(1).map(|m| m.as_str()).unwrap_or(""),
            None => return self.arena.alloc(SyntaxNode::fallback(self.input, begin, limit)),
        };

        let label_end = match REGEX_FOOTNOTE_DEFINITION.find(input_slice) {
            Some(m) => m.end(),
            None => return self.arena.alloc(SyntaxNode::fallback(self.input, begin, limit)),
        };

        let after_label = begin + label_end;
        let line_end_pos = memchr(b'\n', &self.input.as_bytes()[after_label..limit])
            .map_or(limit, |i| after_label + i);

        let mut end = line_end_pos;
        let mut pre_blank: u8 = 0;

        let mut search_pos = line_end_pos;
        while search_pos < limit {
            let remaining = &self.input[search_pos..limit];

            if remaining.starts_with("[fn:") {
                break;
            }

            if remaining.starts_with('*') && remaining.chars().nth(1) == Some(' ') {
                break;
            }

            if remaining.starts_with("\n\n") {
                end = search_pos;
                pre_blank = 2;
                break;
            }

            if let Some(next_line) = remaining.strip_prefix('\n') {
                if next_line.trim().is_empty() {
                    end = search_pos;
                    pre_blank = 1;
                    break;
                }
            }

            if let Some(nl) = memchr(b'\n', remaining.as_bytes()) {
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

        self.arena.alloc(
            SyntaxNode::new(
                Syntax::FootnoteDefinition(Box::new(FootnoteDefinitionData { label, pre_blank, value })),
                (begin, end),
            )
            .content((contents_start, contents_end))
            .post_blank(post_blank)
            .affiliated(affiliated)
            .build()
        )
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
    #[inline]
    pub fn fixed_width_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start: begin, end: limit }, affiliated } = element_span;
        let content_start = self.cursor.pos();
        let mut end_area = content_start;

        while end_area < limit {
            let line_end = memchr(b'\n', &self.input.as_bytes()[end_area..limit])
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

        if end_area == content_start {
            end_area = memchr(b'\n', &self.input.as_bytes()[content_start..limit])
                .map_or(limit, |i| content_start + i + 1);
        }

        // Trim trailing blank lines from the node extent.
        let mut end = end_area;
        while end > content_start {
            match self.input.as_bytes().get(end - 1).copied() {
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => end -= 1,
                _ => break,
            }
        }
        // Include the final newline in the location span.
        if end < end_area {
            end = end_area;
        }

        let raw_value = &self.input[content_start..end_area];

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

        self.arena.alloc(
            SyntaxNode::new(Syntax::FixedWidth(raw_value), (begin, end))
                .post_blank(post_blank)
                .affiliated(affiliated)
                .build()
        )
    }
}
