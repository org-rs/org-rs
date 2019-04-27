// TODO add table related docs

use crate::data::SyntaxNode;
use crate::parser::Parser;

pub struct TableData<'a> {
    /// Formulas associated to the table, if any (string or nil).
    tblfm: Option<&'a str>,
    //Table's origin (symbol table.el, org).
    // type_s

    //Raw table.el table or nil (string or nil).
    // value
}

pub struct TableRowData {
    table_row_type: TableRowType,
}

/// Row's type (symbol standard, rule).
pub enum TableRowType {
    Standard,
    Rule,
}

impl<'a> Parser<'a> {
    // TODO implement table_row_parser
    // https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L2637
    pub fn table_row_parser(&self) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
