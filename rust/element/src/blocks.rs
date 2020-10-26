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
use crate::data::LineNumberingMode;
use crate::data::SyntaxNode;
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    /// Used to identify the  Inline Comments, Blocks, Babel Calls, Dynamic Blocks and Keywords.
    pub static ref REGEX_STARTS_WITH_HASHTAG: Regex = Regex::new(r"[ \t]*#").unwrap();

    /// Used to identify Comments. Used together with REGEX_STARTS_WITH_HASHTAG
    pub static ref REGEX_COLON_OR_EOL: Regex = Regex::new(r"(?: |$)").unwrap();

    /// Used to identify center, comment, example, export, quote, source, verse
    /// and special blocks. Used together with REGEX_STARTS_WITH_HASHTAG
    pub static ref REGEX_BLOCK_BEGIN: Regex = Regex::new(r"\+BEGIN_(\S+)").unwrap();

    pub static ref REGEX_DYNAMIC_BLOCK: Regex = Regex::new(r"\+BEGIN:? ").unwrap();

}

/// Greater element
#[derive(Debug)]
pub struct DynamicBlockData<'a> {
    /// Block's parameters (string).
    arguments: &'a str,

    /// Block's name (string).
    block_name: &'a str,

    /// Drawer's name (string).
    drawer_name: &'a str,
}

#[derive(Debug)]
pub struct CommentBlockData<'a> {
    /// Comments, without block's boundaries (string).
    value: &'a str,
}

#[derive(Debug)]
pub struct ExampleBlockData<'a> {
    /// Format string used to write labels in current block,
    /// if different from org_coderef_label_format (string or nil).
    label_fmt: Option<&'a str>,

    ///Language of the code in the block, if specified (string or nil).
    language: Option<&'a str>,

    /// Non_nil if code lines should be numbered.
    /// A `new` value starts numbering from 1 wheareas
    /// `continued` resume numbering from previous numbered block
    /// (symbol new, continued or nil).
    number_lines: Option<LineNumberingMode>,

    /// Block's options located on the block's opening line (string)
    options: &'a str,

    /// Optional header arguments (string or nil)
    parameters: Option<&'a str>,

    /// Non_nil when indentation within the block mustn't be modified
    /// upon export (boolean).
    preserve_indent: bool,

    /// Non_nil if labels should be kept visible upon export (boolean).
    retain_labels: bool,

    /// Optional switches for code block export (string or nil).
    switches: Option<&'a str>,

    /// Non_nil if links to labels contained in the block should
    /// display the label instead of the line number (boolean).
    use_labels: bool,

    /// Contents (string).
    value: &'a str,
}

#[derive(Debug)]
pub struct ExportBlockData<'a> {
    ///Related back_end's name (string).
    type_s: &'a str,

    ///Contents (string)
    value: &'a str,
}

#[derive(Debug)]
pub struct SpecialBlockData<'a> {
    /// Block's name (string).
    type_s: &'a str,
    /// Raw contents in block (string).
    raw_value: &'a str,
}

#[derive(Debug)]
pub struct SrcBlockData<'a> {
    /// Format string used to write labels in current block,
    /// if different from org_coderef_label_format (string or nil).
    label_fmt: Option<&'a str>,

    /// Language of the code in the block, if specified (string or nil).
    language: Option<&'a str>,

    /// Non_nil if code lines should be numbered.
    /// A `new` value starts numbering from 1 wheareas
    /// `continued` resume numbering from previous
    /// numbered block (symbol new, continued or nil).
    number_lines: Option<LineNumberingMode>,

    /// Optional header arguments (string or nil).
    parameters: Option<&'a str>,

    /// Non_nil when indentation within the block
    /// mustn't be modified upon export (boolean).
    preserve_indent: bool,
    ///Non_nil if labels should be kept visible upon export (boolean).
    retain_labels: bool,

    /// Optional switches for code block export (string or nil).
    switches: Option<&'a str>,

    /// Non_nil if links to labels contained in the block
    /// should display the label instead of the line number (boolean).
    use_labels: bool,

    ///Source code (string).
    value: &'a str,
}

impl<'a> Parser<'a> {
    // TODO implement center_block_parser
    pub fn center_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement comment_block_parser
    pub fn comment_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement comment_block_parser
    pub fn example_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement export_block_parser
    pub fn export_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement quote_block_parser
    pub fn quote_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement src_block_parser
    pub fn src_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement verse_block_parser
    pub fn verse_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement special_block_parser
    pub fn special_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement dynamic_block_parser
    pub fn dynamic_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
