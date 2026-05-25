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

/// Find the bounds of a block that starts at `start` within `input[..limit]`.
///
/// Returns `None` when the `#+BEGIN_` line is the last line (degenerate block).
/// Otherwise returns `Some((after_first, content_end, block_end))`:
///   - `after_first`  — first byte of the block body (after the BEGIN line)
///   - `content_end`  — first byte of the matching `#+END_<type>` line
///   - `block_end`    — first byte past the `#+END_` line
///
/// When no `#+END_` line is found, `content_end == block_end == limit`.
fn find_block_bounds(
    input: &str,
    start: usize,
    limit: usize,
    block_type: &str,
) -> Option<(usize, usize, usize)> {
    let after_first = input[start..limit]
        .find('\n')
        .map_or(limit, |i| start + i + 1);

    if after_first >= limit {
        return None;
    }

    let expected = format!("#+END_{}", block_type.to_ascii_uppercase());
    let mut pos = after_first;
    while pos < limit {
        let line_end = input[pos..limit].find('\n').map_or(limit, |i| pos + i + 1);
        let upper = input[pos..line_end].trim().to_ascii_uppercase();
        if upper == expected
            || upper.starts_with(&format!("{} ", expected))
            || upper.starts_with(&format!("{}\t", expected))
        {
            return Some((after_first, pos, line_end));
        }
        pos = line_end;
    }
    Some((after_first, limit, limit))
}

/// Find bounds of a dynamic block (`#+BEGIN:` / `#+END:`).
///
/// Returns `None` when the `#+BEGIN:` line is the last line.
/// Otherwise returns `Some((after_first, content_end, block_end))`.
fn find_dynamic_block_bounds(
    input: &str,
    start: usize,
    limit: usize,
) -> Option<(usize, usize, usize)> {
    let after_first = input[start..limit]
        .find('\n')
        .map_or(limit, |i| start + i + 1);

    if after_first >= limit {
        return None;
    }

    let mut pos = after_first;
    while pos < limit {
        let line_end = input[pos..limit].find('\n').map_or(limit, |i| pos + i + 1);
        let upper = input[pos..line_end].trim().to_ascii_uppercase();
        if upper.starts_with("#+END:") || upper.starts_with("#+END ") {
            return Some((after_first, pos, line_end));
        }
        pos = line_end;
    }
    Some((after_first, limit, limit))
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
    /// Fallback block parser: consumes from `start` to the matching
    /// `#+END_` line (or `limit`) and returns a Paragraph node.
    fn block_fallback(&self, limit: usize, start: usize, block_type: &str) -> SyntaxNode<'a> {
        let end = find_block_bounds(self.input, start, limit, block_type)
            .map(|(_, _, block_end)| block_end)
            .unwrap_or(limit);
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

    /// Parse a center block element.
    pub fn center_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "CENTER")
        else {
            return self.block_fallback(limit, start, "CENTER");
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::CenterBlock,
            location: Interval { start, end },
            content_location: (after_first < content_end)
                .then_some(Interval { start: after_first, end: content_end }),
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse a comment block element.
    pub fn comment_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "COMMENT")
        else {
            return self.block_fallback(limit, start, "COMMENT");
        };

        let value = &self.input[after_first..content_end];

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::CommentBlock(Box::new(CommentBlockData { value })),
            location: Interval { start, end },
            content_location: None,
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse an example block element.
    pub fn example_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "EXAMPLE")
        else {
            return self.block_fallback(limit, start, "EXAMPLE");
        };

        let value = &self.input[after_first..content_end];

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::ExampleBlock(Box::new(ExampleBlockData {
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
            location: Interval { start, end },
            content_location: None,
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse an export block element.
    pub fn export_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "EXPORT")
        else {
            return self.block_fallback(limit, start, "EXPORT");
        };

        let value = &self.input[after_first..content_end];

        // Extract export backend from first line: "#+BEGIN_EXPORT html"
        let first_line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i);
        let first_line = &self.input[start..first_line_end];
        let type_s = first_line.split_whitespace().nth(1).unwrap_or("html");

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::ExportBlock(Box::new(ExportBlockData { type_s, value })),
            location: Interval { start, end },
            content_location: None,
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse a quote block element.
    pub fn quote_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "QUOTE")
        else {
            return self.block_fallback(limit, start, "QUOTE");
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::QuoteBlock,
            location: Interval { start, end },
            content_location: (after_first < content_end)
                .then_some(Interval { start: after_first, end: content_end }),
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse a src block element.
    pub fn src_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "SRC")
        else {
            return self.block_fallback(limit, start, "SRC");
        };

        let value = &self.input[after_first..content_end];

        // Extract language from first line: "#+BEGIN_SRC python"
        let first_line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i);
        let first_line = &self.input[start..first_line_end];
        let language = first_line.split_whitespace().nth(1);

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::SrcBlock(Box::new(SrcBlockData {
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
            location: Interval { start, end },
            content_location: None,
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse a verse block element.
    pub fn verse_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, "VERSE")
        else {
            return self.block_fallback(limit, start, "VERSE");
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::VerseBlock,
            location: Interval { start, end },
            content_location: (after_first < content_end)
                .then_some(Interval { start: after_first, end: content_end }),
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Parse a special block element.
    pub fn special_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        // Extract block type from #+BEGIN_NAME before bounding the block.
        let first_line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i);
        let first_line = &self.input[start..first_line_end];
        let type_s = REGEX_BLOCK_BEGIN
            .captures(first_line)
            .and_then(|c| c.get(1))
            .map_or("special", |m| m.as_str());

        let Some((after_first, content_end, end)) =
            find_block_bounds(self.input, start, limit, type_s)
        else {
            return self.block_fallback(limit, start, type_s);
        };

        let raw_value = &self.input[after_first..content_end];

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::SpecialBlock(Box::new(SpecialBlockData { type_s, raw_value })),
            location: Interval { start, end },
            content_location: (after_first < content_end)
                .then_some(Interval { start: after_first, end: content_end }),
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }

    /// Fallback: dynamic block parser (not yet fully implemented).
    pub fn dynamic_block_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let Some((after_first, content_end, end)) =
            find_dynamic_block_bounds(self.input, start, limit)
        else {
            // Degenerate: BEGIN: is the last line; consume to limit as paragraph.
            return SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(vec![]),
                data: Syntax::Paragraph,
                location: Interval { start, end: limit },
                content_location: None,
                post_blank: 0,
                affiliated: None,
            };
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::DynamicBlock(Box::new(DynamicBlockData {
                arguments: "",
                block_name: "",
                drawer_name: "",
            })),
            location: Interval { start, end },
            content_location: (after_first < content_end)
                .then_some(Interval { start: after_first, end: content_end }),
            post_blank: post_blank(self.input, end, limit),
            affiliated,
        }
    }
}
