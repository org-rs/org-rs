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
