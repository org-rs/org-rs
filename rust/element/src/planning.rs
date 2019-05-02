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

use crate::cursor::CursorHelper;
use crate::data::SyntaxNode;
use crate::parser::Parser;

impl<'a> Parser<'a> {
    // TODO implement planning_parser
    pub fn planning_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
    // TODO implement clock_line_parser
    pub fn clock_line_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
