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

    /// Regular expression matching the definition of a footnote.
    /// Match group 1 contains definition's label
    pub static ref REGEX_FOOTNOTE_DEFINITION: Regex = Regex::new(r"^\[fn:([-_[:word:]]+)\]").unwrap();
}

/// Greater element
pub struct FootnoteDefinitionData<'a> {
    /// Label used for references (string).
    label: &'a str,

    /// Number of newline characters between the
    /// beginning of the footnoote and the beginning
    /// of the contents (0, 1 or 2).
    pre_blank: u8,
}

impl<'a> Parser<'a> {
    // TODO implement footnote_definition_parser
    pub fn footnote_definition_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
