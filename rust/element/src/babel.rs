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

use crate::affiliated::AffiliatedData;
use crate::data::SyntaxNode;
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_BABEL_CALL: Regex = Regex::new(r"\+CALL:").unwrap();
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

impl<'a> Parser<'a> {
    // TODO implement babel_call_parser
    pub fn babel_call_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
