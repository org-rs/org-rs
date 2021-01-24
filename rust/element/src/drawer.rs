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


    /// Matches first or last line of a drawer
    /// Group 1 contains drawer's name or \"END\"
    pub static ref REGEX_DRAWER: Regex = Regex::new(r"^[ \t]*:((?:\w|[-_])+):[ \t]*$").unwrap();

}

#[derive(Debug)]
pub struct DrawerData<'a> {
    /// Drawer's name (string).
    pub drawer_name: &'a str,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    // TODO implement drawer_parser
    pub fn drawer_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
