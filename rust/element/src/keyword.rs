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

use crate::affiliated::AffiliatedData;
use crate::data::SyntaxNode;
use crate::data::SyntaxT;
use crate::parser::Parser;
use regex::{Match, Regex};
use std::borrow::Cow;

lazy_static! {
    pub static ref REGEX_KEYWORD: Regex = Regex::new(r"\+\S+:").unwrap();
}

#[derive(Debug)]
pub struct KeywordData<'a> {
    /// Keyword's name (string).
    key: &'a str,
    /// Keyword's value (string).
    value: &'a str,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    // TODO implement keyword_parser
    /// Parse a keyword at point.
    ///
    /// LIMIT bounds the search.  AFFILIATED is a list of which CAR is
    /// the buffer position at the beginning of the first affiliated
    /// keyword and CDR is a plist of affiliated keywords along with
    /// their value.
    ///
    /// Return a list whose CAR is `keyword' and CDR is a plist
    /// containing `:key', `:value', `:begin', `:end', `:post-blank' and
    /// `:post-affiliated' keywords."

    pub fn keyword_parser(
        &self,
        limit: usize,
        start: usize,
        maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        // (save-excursion
        //   ;; An orphaned affiliated keyword is considered as a regular
        //   ;; keyword.  In this case AFFILIATED is nil, so we take care of
        //   ;; this corner case.
        unimplemented!()
    }
}
