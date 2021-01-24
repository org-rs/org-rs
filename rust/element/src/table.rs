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

// TODO add table related docs

use crate::affiliated::AffiliatedData;
use crate::data::SyntaxNode;
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_TABLE_BORDER: Regex = Regex::new(r"[ \t]*\|").unwrap();
    pub static ref REGEX_TABLE_RULE: Regex = Regex::new(r"[ \t]*\+(-+\+)+[ \t]*$").unwrap();
    pub static ref REGEX_TABLE_PRE_BORDER: Regex = Regex::new(r"^[ \t]*($|[^|])").unwrap();
}

#[derive(Debug)]
pub struct TableData<'a> {
    /// Formulas associated to the table, if any (string or nil).
    tblfm: Option<&'a str>,
    //Table's origin (symbol table.el, org).
    // type_s

    //Raw table.el table or nil (string or nil).
    // value
}

#[derive(Debug)]
pub struct TableRowData {
    table_row_type: TableRowType,
}

/// Row's type (symbol standard, rule).
#[derive(Debug)]
pub enum TableRowType {
    Standard,
    Rule,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    // TODO implement table_row_parser
    // https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L2637
    pub fn table_row_parser(&self) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement table_parser
    pub fn table_parser(
        &self,
        limit: usize,
        start: usize,
        maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
