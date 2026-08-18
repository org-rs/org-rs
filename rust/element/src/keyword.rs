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
use crate::cursor::CachedRegex;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_KEYWORD: CachedRegex = CachedRegex::new(Regex::new(r"\+\S+:").unwrap());
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
    pub fn keyword_parser(
        &self,
        limit: usize,
        start: usize,
        _maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i + 1);

        let line = &self.input[start..line_end].trim_end();

        // Strip leading whitespace and the `#+` prefix.
        let stripped = line.trim_start();
        let after_hash = if stripped.starts_with("#+") {
            &stripped[2..]
        } else {
            stripped
        };

        // Split at first `:` to get KEY and VALUE.
        let (key, value) = match after_hash.find(':') {
            Some(i) => {
                let k = &after_hash[..i];
                let v = after_hash.get(i + 1..).unwrap_or("").trim_start();
                (k, v)
            }
            None => (after_hash, ""),
        };

        // Find key and value as slices of self.input so lifetimes work.
        let key_offset = key.as_ptr() as usize - self.input.as_ptr() as usize;
        let key_ref = &self.input[key_offset..key_offset + key.len()];
        let (value_ref, value_offset) = if value.is_empty() {
            // Empty value may be a static "" literal, not a subslice of input.
            ("", 0)
        } else {
            let off = value.as_ptr() as usize - self.input.as_ptr() as usize;
            (&self.input[off..off + value.len()], off)
        };
        let _ = value_offset; // suppress unused warning

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Keyword(Box::new(KeywordData {
                key: key_ref,
                value: value_ref,
            })),
            location: Interval {
                start,
                end: line_end,
            },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }
}
