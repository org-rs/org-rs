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

use std::cell::RefCell;

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, LineNumberingMode, Syntax, SyntaxNode};
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

/// Find the end of a block that starts at `start` within `input[..limit]`.
///
/// Scans forward for a line whose trimmed, uppercased content starts with
/// `#+END_` and returns the byte position just past that line.  Falls back
/// to `limit` when no closing line is found.
fn find_block_end(input: &str, start: usize, limit: usize) -> usize {
    // Skip the opening #+BEGIN_ line first.
    let after_first = input[start..limit]
        .find('\n')
        .map_or(limit, |i| start + i + 1);

    let mut pos = after_first;
    while pos < limit {
        let line_end = input[pos..limit].find('\n').map_or(limit, |i| pos + i + 1);
        let trimmed = input[pos..line_end].trim();
        if trimmed.len() >= 6 {
            let upper: String = trimmed
                .chars()
                .take(6)
                .collect::<String>()
                .to_ascii_uppercase();
            if upper == "#+END_" {
                return line_end;
            }
        }
        pos = line_end;
    }
    limit
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Fallback block parser: consumes from `start` to the matching
    /// `#+END_` line (or `limit`) and returns a Paragraph node.
    fn block_fallback(&self, limit: usize, start: usize) -> SyntaxNode<'a> {
        let end = find_block_end(self.input, start, limit);
        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Paragraph,
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Fallback: center block parser (not yet fully implemented).
    pub fn center_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: comment block parser (not yet fully implemented).
    pub fn comment_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: example block parser (not yet fully implemented).
    pub fn example_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: export block parser (not yet fully implemented).
    pub fn export_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: quote block parser (not yet fully implemented).
    pub fn quote_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: src block parser (not yet fully implemented).
    pub fn src_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: verse block parser (not yet fully implemented).
    pub fn verse_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: special block parser (not yet fully implemented).
    pub fn special_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        self.block_fallback(limit, start)
    }

    /// Fallback: dynamic block parser (not yet fully implemented).
    pub fn dynamic_block_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        // Dynamic blocks use #+BEGIN: / #+END: (no underscore after END).
        let after_first = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i + 1);
        let mut pos = after_first;
        let end = loop {
            if pos >= limit {
                break limit;
            }
            let line_end = self.input[pos..limit]
                .find('\n')
                .map_or(limit, |i| pos + i + 1);
            let trimmed = self.input[pos..line_end].trim();
            let upper = trimmed.to_ascii_uppercase();
            if upper.starts_with("#+END:") || upper.starts_with("#+END ") {
                break line_end;
            }
            pos = line_end;
        };
        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Paragraph,
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }
}
