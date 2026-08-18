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
//!   `root/.../plain_list/item/paragraph` — extra-in-rust / missing-in-rust,
//!   the paragraph `:begin` differing from the oracle by one byte.
//!
//! When a list bullet is followed by more than one space (e.g. `-  text`),
//! `item_parser_internal` computed the item's content start by adding a
//! hard-coded single trailing space to the bullet (`+ 1`).  Emacs instead
//! skips *all* whitespace after the bullet marker (its `org-list-full-item-re`
//! match is followed by `skip-chars-forward " \r\t\n"`), so the paragraph
//! begins one byte later.  Every list item written with aligned, multi-space
//! bullets therefore had its paragraph `:begin` shifted one byte too early,
//! cascading into a missing/extra paragraph pair per item in the oracle
//! comparison.
//!
//! Corpus origin: alphapapa_org_super_agenda.org — the table-of-contents and
//! example lists use `-  [[...]]` (dash followed by two spaces) throughout,
//! producing 132 paragraph discrepancies all from this single cause.
use bumpalo::Bump;
use org_element::data::{Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use org_element::testutils::*;

/// Locate the first `Item`'s first child `Paragraph` and return its
/// `(start, end)` byte location.
fn first_item_paragraph_span(input: &str) -> (usize, usize) {
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let section = arena[root].children[0];
    let list = arena[section].children[0];
    let item = arena[list].children[0];
    let Syntax::Item(_) = &arena[item].data else {
        panic!("expected Item, got {:?}", arena[item].data);
    };
    let para = arena[item].children[0];
    let Syntax::Paragraph = &arena[para].data else {
        panic!("expected Paragraph, got {:?}", arena[para].data);
    };
    (arena[para].location.start, arena[para].location.end)
}

/// A bullet followed by two spaces: the paragraph must begin after *both*
/// spaces, matching Emacs' `:begin` (1-based 4 -> 0-based byte offset 3).
#[test]
fn two_space_bullet_paragraph_begins_after_all_whitespace() {
    let input = "-  two spaces\n";
    //            0123
    //            ^ '-' at 0, spaces at 1 and 2, 't' at 3
    let (start, _end) = first_item_paragraph_span(input);
    assert_eq!(
        start, 3,
        "paragraph must begin at the first non-whitespace char after the bullet \
         (byte 3, the 't'), not inside the bullet's trailing whitespace"
    );
}

/// A single-space bullet must be unaffected by the fix: content begins right
/// after the one space, as before.
#[test]
fn single_space_bullet_unchanged() {
    let input = "- one space\n";
    //            01
    //            ^ '-' at 0, space at 1, 'o' at 2
    let (start, _end) = first_item_paragraph_span(input);
    assert_eq!(start, 2, "single-space bullet content begins at byte 2");
}

/// Four spaces after the bullet: paragraph begins at the text, not in the run
/// of spaces.
#[test]
fn many_space_bullet_paragraph_begins_at_text() {
    let input = "-    wide gap\n";
    //            0    5
    //            ^ '-' at 0, spaces at 1..=4, 'w' at 5
    let (start, _end) = first_item_paragraph_span(input);
    assert_eq!(
        start, 5,
        "paragraph begins at the text after all four spaces"
    );

    // Sanity: exactly one paragraph parsed for the single item.
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    assert_eq!(count_type(&arena, root, SyntaxT::Paragraph), 1);
}
