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

/// Parse a paragraph.
///
/// LIMIT bounds the search.  AFFILIATED is a list of which CAR is
/// the buffer position at the beginning of the first affiliated
/// keyword and CDR is a plist of affiliated keywords along with
/// their value.
///
/// Return a list whose CAR is `paragraph' and CDR is a plist
/// containing `:begin', `:end', `:contents-begin' and
/// `:contents-end', `:post-blank' and `:post-affiliated' keywords.
///
/// Assume point is at the beginning of the paragraph."
/// (defun org-element-paragraph-parser (limit affiliated)
impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    // TODO implement paragraph_parser
    pub fn paragraph_parser(
        &self,
        limit: usize,
        start: usize,
        maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement section_parser
    pub fn section_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
