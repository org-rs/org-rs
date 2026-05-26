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

use crate::affiliated::ElementSpan;
use crate::data::{Interval, LineNumberingMode, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use memchr::memchr;
use regex::Regex;

lazy_static! {
    /// Used to identify the  Inline Comments, Blocks, Babel Calls, Dynamic Blocks and Keywords.
    pub static ref REGEX_STARTS_WITH_HASHTAG: Regex = Regex::new(r"[ \t]*#").unwrap();

    /// Used to identify Comments. Used together with REGEX_STARTS_WITH_HASHTAG
    pub static ref REGEX_COLON_OR_EOL: Regex = Regex::new(r"(?: |$)").unwrap();

    /// Used to identify center, comment, example, export, quote, source, verse
    /// and special blocks. Used together with REGEX_STARTS_WITH_HASHTAG
    /// Case insensitive to match #+BEGIN_CENTER and #+begin_center
    pub static ref REGEX_BLOCK_BEGIN: Regex = Regex::new(r"(?i)\+BEGIN_(\S+)").unwrap();

    /// Used to identify rare, but technically legal dynamic `BEGIN` blocks
    /// Note: uses #+BEGIN: (no underscore after BEGIN)
    pub static ref REGEX_DYNAMIC_BLOCK: Regex = Regex::new(r"(?i)\+BEGIN:? ").unwrap();
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
    pub type_s: &'a str,

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
    pub language: Option<&'a str>,

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

/// Spans for a parsed block.
///
/// `location` covers the whole block from `#+BEGIN_` through the end of the
/// `#+END_` line.  `content` covers the lines between those delimiters.
pub struct BlockBounds {
    pub location: Interval,
    pub content: Interval,
}

/// Shared iterator that scans lines from `content_start` until `is_end` matches.
///
/// Returns `None` when the opening line is the last line (degenerate block).
/// When no end line is found the entire remaining span is treated as content.
fn find_block_bounds_impl(
    input: &str,
    start: usize,
    limit: usize,
    is_end: impl Fn(&str) -> bool,
) -> Option<BlockBounds> {
    let content_start = memchr(b'\n', input[start..limit].as_bytes())
        .map_or(limit, |i| start + i + 1);

    if content_start >= limit {
        return None;
    }

    let mut pos = content_start;
    while pos < limit {
        let line_end = memchr(b'\n', input[pos..limit].as_bytes()).map_or(limit, |i| pos + i + 1);
        if is_end(&input[pos..line_end]) {
            return Some(BlockBounds {
                location: Interval { start, end: line_end },
                content: Interval { start: content_start, end: pos },
            });
        }
        pos = line_end;
    }
    Some(BlockBounds {
        location: Interval { start, end: limit },
        content: Interval { start: content_start, end: limit },
    })
}

/// Find the bounds of a named block (`#+BEGIN_TYPE` / `#+END_TYPE`).
fn find_block_bounds(input: &str, start: usize, limit: usize, block_type: &str) -> Option<BlockBounds> {
    let end_tag = b"#+END_";
    find_block_bounds_impl(input, start, limit, |line| {
        let trimmed = line.trim();
        let bytes = trimmed.as_bytes();
        let tag_len = end_tag.len() + block_type.len();
        if bytes.len() < tag_len {
            return false;
        }
        if !bytes[..end_tag.len()].eq_ignore_ascii_case(end_tag) {
            return false;
        }
        if !bytes[end_tag.len()..tag_len].eq_ignore_ascii_case(block_type.as_bytes()) {
            return false;
        }
        bytes.len() == tag_len || bytes[tag_len] == b' ' || bytes[tag_len] == b'\t'
    })
}

/// Find bounds of a dynamic block (`#+BEGIN:` / `#+END:`).
fn find_dynamic_block_bounds(input: &str, start: usize, limit: usize) -> Option<BlockBounds> {
    find_block_bounds_impl(input, start, limit, |line| {
        let trimmed = line.trim();
        let bytes = trimmed.as_bytes();
        bytes.len() >= 6
            && bytes[..5].eq_ignore_ascii_case(b"#+END")
            && (bytes[5] == b':' || bytes[5] == b' ')
    })
}

fn post_blank(input: &str, end: usize, limit: usize) -> usize {
    if end < limit {
        let remaining = &input[end..limit];
        let trimmed = remaining.trim_start();
        (remaining.len() - trimmed.len()).min(2)
    } else {
        0
    }
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Fallback: consume from `start` to the matching `#+END_` line (or
    /// `limit`) and return a `Paragraph` node so parsing can continue.
    fn block_fallback(&mut self, span: Interval, block_type: &str) -> NodeId {
        let end = find_block_bounds(self.input, span.start, span.end, block_type)
            .map(|b| b.location.end)
            .unwrap_or(span.end);
        self.arena.alloc(SyntaxNode::new(Syntax::Paragraph, (span.start, end)).build())
    }

    /// Shared implementation for the three content-only blocks (CENTER, QUOTE, VERSE):
    /// blocks whose only parse output is a location, content span, and affiliated data.
    fn parse_content_block(
        &mut self,
        element_span: ElementSpan<'a>,
        tag: &str,
        syntax: Syntax<'a>,
    ) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, tag) else {
            return self.block_fallback(Interval { start, end: limit }, tag);
        };
        self.arena.alloc(SyntaxNode::new(syntax, bounds.location)
            .content(bounds.content)
            .post_blank(post_blank(self.input, bounds.location.end, limit))
            .affiliated(affiliated)
            .build())
    }

    /// Parse a center block element.
    pub fn center_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        self.parse_content_block(element_span, "CENTER", Syntax::CenterBlock)
    }

    /// Parse a comment block element.
    pub fn comment_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, "COMMENT") else {
            return self.block_fallback(Interval { start, end: limit }, "COMMENT");
        };
        let value = &self.input[bounds.content.start..bounds.content.end];
        self.arena.alloc(SyntaxNode::new(Syntax::CommentBlock(value), bounds.location)
            .post_blank(post_blank(self.input, bounds.location.end, limit))
            .affiliated(affiliated)
            .build())
    }

    /// Parse an example block element.
    pub fn example_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, "EXAMPLE") else {
            return self.block_fallback(Interval { start, end: limit }, "EXAMPLE");
        };
        let value = &self.input[bounds.content.start..bounds.content.end];
        self.arena.alloc(SyntaxNode::new(
            Syntax::ExampleBlock(Box::new(ExampleBlockData {
                label_fmt: None,
                language: None,
                number_lines: None,
                options: "",
                parameters: None,
                preserve_indent: false,
                retain_labels: false,
                switches: None,
                use_labels: false,
                value,
            })),
            bounds.location,
        )
        .post_blank(post_blank(self.input, bounds.location.end, limit))
        .affiliated(affiliated)
        .build())
    }

    /// Parse an export block element.
    pub fn export_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, "EXPORT") else {
            return self.block_fallback(Interval { start, end: limit }, "EXPORT");
        };
        let value = &self.input[bounds.content.start..bounds.content.end];
        let first_line_end = memchr(b'\n', self.input[start..limit].as_bytes()).map_or(limit, |i| start + i);
        let type_s = self.input[start..first_line_end].split_whitespace().nth(1).unwrap_or("html");
        self.arena.alloc(SyntaxNode::new(
            Syntax::ExportBlock(Box::new(ExportBlockData { type_s, value })),
            bounds.location,
        )
        .post_blank(post_blank(self.input, bounds.location.end, limit))
        .affiliated(affiliated)
        .build())
    }

    /// Parse a quote block element.
    pub fn quote_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, "QUOTE") else {
            return self.block_fallback(Interval { start, end: limit }, "QUOTE");
        };
        self.arena.alloc(SyntaxNode::new(Syntax::QuoteBlock, bounds.location)
            .content(bounds.content)
            .post_blank(post_blank(self.input, bounds.location.end, limit))
            .affiliated(affiliated)
            .build())
    }

    /// Parse a src block element.
    pub fn src_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, "SRC") else {
            return self.block_fallback(Interval { start, end: limit }, "SRC");
        };
        let value = &self.input[bounds.content.start..bounds.content.end];
        let first_line_end = memchr(b'\n', self.input[start..limit].as_bytes()).map_or(limit, |i| start + i);
        let language = self.input[start..first_line_end].split_whitespace().nth(1);
        self.arena.alloc(SyntaxNode::new(
            Syntax::SrcBlock(Box::new(SrcBlockData {
                label_fmt: None,
                language,
                number_lines: None,
                parameters: None,
                preserve_indent: false,
                retain_labels: false,
                switches: None,
                use_labels: false,
                value,
            })),
            bounds.location,
        )
        .post_blank(post_blank(self.input, bounds.location.end, limit))
        .affiliated(affiliated)
        .build())
    }

    /// Parse a verse block element.
    pub fn verse_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_block_bounds(self.input, start, limit, "VERSE") else {
            return self.block_fallback(Interval { start, end: limit }, "VERSE");
        };
        self.arena.alloc(SyntaxNode::new(Syntax::VerseBlock, bounds.location)
            .content(bounds.content)
            .post_blank(post_blank(self.input, bounds.location.end, limit))
            .affiliated(affiliated)
            .build())
    }

    /// Parse a special block element.
    pub fn special_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let first_line_end = memchr(b'\n', self.input[start..limit].as_bytes()).map_or(limit, |i| start + i);
        let type_s = REGEX_BLOCK_BEGIN
            .captures(&self.input[start..first_line_end])
            .and_then(|c| c.get(1))
            .map_or("special", |m| m.as_str());
        let Some(bounds) = find_block_bounds(self.input, start, limit, type_s) else {
            return self.block_fallback(Interval { start, end: limit }, type_s);
        };
        let raw_value = &self.input[bounds.content.start..bounds.content.end];
        self.arena.alloc(SyntaxNode::new(
            Syntax::SpecialBlock(Box::new(SpecialBlockData { type_s, raw_value })),
            bounds.location,
        )
        .content(bounds.content)
        .post_blank(post_blank(self.input, bounds.location.end, limit))
        .affiliated(affiliated)
        .build())
    }

    /// Fallback: dynamic block parser (not yet fully implemented).
    pub fn dynamic_block_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let ElementSpan { span: Interval { start, end: limit }, affiliated } = element_span;
        let Some(bounds) = find_dynamic_block_bounds(self.input, start, limit) else {
            return self.arena.alloc(SyntaxNode::new(Syntax::Paragraph, (start, limit)).build());
        };
        self.arena.alloc(SyntaxNode::new(
            Syntax::DynamicBlock(Box::new(DynamicBlockData {
                arguments: "",
                block_name: "",
                drawer_name: "",
            })),
            bounds.location,
        )
        .content(bounds.content)
        .post_blank(post_blank(self.input, bounds.location.end, limit))
        .affiliated(affiliated)
        .build())
    }
}
