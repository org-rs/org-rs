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
use crate::markup::REGEX_DIARY_SEXP;
use crate::parser::Parser;

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Fallback: planning parser (not yet fully implemented).
    pub fn planning_parser(&self, limit: usize) -> SyntaxNode<'a> {
        let start = self.cursor.borrow().pos();
        SyntaxNode::fallback(self.input, start, limit)
    }

    /// Fallback: clock line parser (not yet fully implemented).
    pub fn clock_line_parser(&self, limit: usize) -> SyntaxNode<'a> {
        let start = self.cursor.borrow().pos();
        SyntaxNode::fallback(self.input, start, limit)
    }

    /// Parse a diary sexp element.
    ///
    /// Diary sexp format: `%%(SEXP)` at beginning of line (unindented).
    /// The sexp must have balanced parentheses.
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound search
    /// - Uses `start` for begin position
    /// - Value is the full sexp string
    /// Parse a diary sexp element.
    ///
    /// Diary sexp format: `%%(SEXP)` at beginning of line (unindented).
    /// The sexp must have balanced parentheses.
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound search
    /// - Uses `start` for begin position
    /// - Value is the full sexp string
    /// - Stores affiliated data in SyntaxNode
    pub fn diary_sexp_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let input_slice = &self.input[start..limit];

        let caps = match REGEX_DIARY_SEXP.captures(input_slice) {
            Some(c) => c,
            None => return SyntaxNode::fallback(self.input, start, limit),
        };

        let value = match caps.get(1) {
            Some(m) => m.as_str(),
            None => return SyntaxNode::fallback(self.input, start, limit),
        };

        let line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i);

        let end = line_end;

        let post_blank = if end < limit {
            let remaining = &self.input[end..limit];
            let trimmed = remaining.trim_start();
            (remaining.len() - trimmed.len()).min(2)
        } else {
            0
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::DiarySexp(Box::new(crate::data::DiarySexpData { value })),
            location: Interval { start, end },
            content_location: None,
            post_blank,
            affiliated,
        }
    }
}
