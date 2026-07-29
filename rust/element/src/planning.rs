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
use crate::cursor::CachedRegex;
use crate::data::SyntaxNode;
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_DIARY_SEXP: CachedRegex = CachedRegex::new(Regex::new(r"%%\(").unwrap());
}

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

    /// Fallback: diary sexp parser (not yet fully implemented).
    pub fn diary_sexp_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        SyntaxNode::fallback(self.input, start, limit)
    }
}
