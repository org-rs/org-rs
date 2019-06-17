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
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    // TODO check if needed and remove if not
    pub static ref REGEX_: Regex = Regex::new(r"").unwrap();
}

pub struct CommentData<'a> {
    /// Comments, with pound signs (string).
    value: &'a str,
}

impl<'a> Parser<'a> {
    // TODO implement drawer_parser
    pub fn comment_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
