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
impl<'a> Parser<'a> {
    // TODO implement paragraph_parser
    pub fn paragraph_parser(&self, limit: usize, affiliated_start: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
