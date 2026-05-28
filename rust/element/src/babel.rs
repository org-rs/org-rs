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
use memchr::memchr;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_BABEL_CALL: CachedRegex =
        CachedRegex::new(Regex::new(r"(?i)\+CALL:").unwrap());
}

#[derive(Debug)]
pub struct BabelCallData<'a> {
    /// Name of code block being called (string).
    pub call: &'a str,

    /// Header arguments applied to the named code block (string or nil).
    pub inside_header: Option<&'a str>,

    /// Arguments passed to the code block (string or nil).
    pub arguments: Option<&'a str>,

    /// Header arguments applied to the calling instance (string or nil).
    pub end_header: Option<&'a str>,

    /// Raw call, as Org syntax (string).
    pub value: &'a str,
}

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    /// Parse a babel call element.
    ///
    /// Format: `#+CALL: name(args)` or `#+CALL: name[:header] args`
    /// Case insensitive (matches CALL, call, etc.)
    #[inline]
    pub fn babel_call_parser(&mut self, element_span: ElementSpan<'a, 'b>) -> NodeId {
        let ElementSpan {
            span: Interval { start, end: limit },
            affiliated,
        } = element_span;
        let input_slice = &self.input[start..limit];

        if !REGEX_BABEL_CALL.is_match(input_slice) {
            return self
                .arena
                .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump));
        }

        let line_end = memchr(b'\n', input_slice.as_bytes()).map_or(limit, |i| start + i);
        let value = &self.input[start..line_end];

        let call_name = REGEX_BABEL_CALL
            .find(input_slice)
            .map(|m| {
                input_slice[m.end()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
            })
            .unwrap_or("");

        let post_blank = if line_end < limit {
            let remaining = &self.input[line_end..limit];
            remaining.len() - remaining.trim_start().len()
        } else {
            0
        }
        .min(2);

        self.arena.alloc(
            SyntaxNode::new(
                Syntax::BabelCall(self.bump.alloc(BabelCallData {
                    call: call_name,
                    inside_header: None,
                    arguments: None,
                    end_header: None,
                    value,
                })),
                (start, line_end),
                self.bump,
            )
            .post_blank(post_blank)
            .affiliated(affiliated)
            .build(),
        )
    }
}
