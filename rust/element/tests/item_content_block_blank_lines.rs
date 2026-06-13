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

//! Tests for the parser - adapted from Emacs org-element test suite
//!
//! Corpus discrepancy class:
//!   `root/.../plain_list/item/{paragraph,src_block}` — missing in Rust when a
//!   block body inside a list item contains two consecutive blank lines.
//!
//! Two consecutive blank lines normally terminate a list item, but NOT when
//! they appear inside a `#+BEGIN_`/`#+END_` block.  `item_content_end` scanned
//! line-by-line without tracking block boundaries, so the first blank line of
//! a `\n\n` pair inside a block body was mistaken for the item-content
//! terminator.  Everything after that block (further paragraphs, source
//! blocks, sibling items) was silently dropped.  `list_struct` already had
//! block-aware skipping; `item_content_end` did not.
//!
//! Corpus origin: emacs_china_org_manual.org @270771 (the `可索引的变量值`
//! list item, whose `#+BEGIN_SRC org` body contains a `\n\n` blank pair).
mod common;
use bumpalo::Bump;
use common::*;
use org_element::data::SyntaxT;
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

/// A single list item whose source block contains two consecutive blank
/// lines, followed by a second paragraph and a second source block.  All of
/// the item's content must survive parsing.
#[test]
fn blank_lines_inside_block_do_not_truncate_item_content() {
    // Note the `\n\n` blank pair between `line a` and `line b`, which lives
    // inside the first block's body.
    let input = "- item\n\n  para1\n  #+BEGIN_SRC org\n    line a\n\n\n    line b\n  #+END_SRC\n\n  para2\n  #+BEGIN_SRC org\n    body2\n  #+END_SRC\n";

    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let paragraphs = count_type(&arena, root, SyntaxT::Paragraph);
    let src_blocks = count_type(&arena, root, SyntaxT::SrcBlock);

    assert_eq!(
        src_blocks, 2,
        "both source blocks inside the item must be parsed; got {src_blocks} \
         (the blank pair inside the first block body truncated the item)"
    );
    // Three paragraphs: the bullet line `item`, `para1` and `para2`.
    assert_eq!(
        paragraphs, 3,
        "every paragraph inside the item must be parsed; got {paragraphs}"
    );
}

/// The blank pair inside a block must not split a single list item into two
/// lists either: the trailing content stays inside one item / one list.
#[test]
fn blank_lines_inside_block_keep_single_list() {
    let input = "- item\n\n  para1\n  #+BEGIN_SRC org\n    a\n\n\n    b\n  #+END_SRC\n\n  para2\n";

    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let lists = count_type(&arena, root, SyntaxT::PlainList);
    let items = count_type(&arena, root, SyntaxT::Item);
    assert_eq!(lists, 1, "expected a single PlainList, got {lists}");
    assert_eq!(items, 1, "expected a single Item, got {items}");
    assert_eq!(
        count_type(&arena, root, SyntaxT::SrcBlock),
        1,
        "the source block must remain inside the item"
    );
    // The bullet line `item`, plus `para1` and `para2`.
    assert_eq!(
        count_type(&arena, root, SyntaxT::Paragraph),
        3,
        "paragraphs before and after the block must be present"
    );
}
