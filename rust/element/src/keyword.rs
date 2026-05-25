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
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_KEYWORD: Regex = Regex::new(r"\+\S+:").unwrap();
}

#[derive(Debug)]
pub struct KeywordData<'a> {
    /// Keyword's name (string).
    pub key: &'a str,
    /// Keyword's value (string).
    pub value: &'a str,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Parse a keyword at point.
    ///
    /// A keyword follows the pattern `#+KEY: VALUE`.  `start` is the
    /// buffer position at the beginning of the first affiliated keyword
    /// (or the keyword itself when there is no affiliation).
    pub fn keyword_parser(&self, element_span: ElementSpan<'a>) -> SyntaxNode<'a> {
        let ElementSpan { span: Interval { start, end: limit }, affiliated: _ } = element_span;
        let line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i + 1);

        let line = &self.input[start..line_end].trim_end();

        let stripped = line.trim_start();
        let after_hash = if stripped.starts_with("#+") { &stripped[2..] } else { stripped };

        let (key, value) = match after_hash.find(':') {
            Some(i) => (&after_hash[..i], after_hash.get(i + 1..).unwrap_or("").trim_start()),
            None => (after_hash, ""),
        };

        let key_offset = key.as_ptr() as usize - self.input.as_ptr() as usize;
        let key_ref = &self.input[key_offset..key_offset + key.len()];
        let value_ref = if value.is_empty() {
            ""
        } else {
            let off = value.as_ptr() as usize - self.input.as_ptr() as usize;
            &self.input[off..off + value.len()]
        };

        SyntaxNode::new(
            Syntax::Keyword(Box::new(KeywordData { key: key_ref, value: value_ref })),
            (start, line_end),
        )
        .build()
    }
}
