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
//! This module contains tests for unimplemented or incomplete features
//! based on the Emacs test-org-element.el test suite.

use org_element::testutils::*;

#[test]
fn simple_item() {
    for (input, desc) in &[
        ("- some text\n", "dash bullet"),
        ("+ some text\n", "plus bullet"),
        ("- tag :: content\n", "tag item"),
    ] {
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 item ({}), found {}", desc, count);
    }
}

#[test]
fn item_with_checkbox() {
    for (input, desc) in &[
        ("- [X] checked item\n", "[X]"),
        ("- [-] in-progress item\n", "[-]"),
        ("- [ ] unchecked item\n", "[ ]"),
    ] {
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 item with {} checkbox, found {}",
            desc, count
        );
    }
}

#[test]
fn item_ordered_start() {
    let input = "1. first item\n2. second item\n";
    let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
    assert_eq!(count, 2, "Expected 2 items, found {}", count);
}

#[test]
fn item_with_counter_set() {
    let input = "1. [@3] item\n2. next\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let list_node = section_children.first().expect("Expected plain list");
    let list_children = &arena[*list_node].children;
    let first_item = list_children.first().expect("Expected first item");

    let Syntax::Item(data) = &arena[*first_item].data else {
        panic!("Expected Item, got: {:?}", arena[*first_item].data);
    };
    assert_eq!(
        data.counter, 3,
        "Item counter should be 3 from [@3], got {}",
        data.counter
    );
}

/// A headline following a plain list (after a blank line) must NOT be
/// parsed as a child of the last list item.  Emacs `list_struct` stops
/// the scan at the headline line; the item's `:contents-end` is therefore
/// the start of that headline, not the section limit.
///
/// Discrepancy class: `root/headline/section/plain_list/item/headline`
///
/// Corner cases:
/// - headline directly after a blank line following an item
/// - same-level sub-headline after a flat list inside a heading
/// - multiple items, headline follows the last one
mod headline_not_inside_item {
    use super::*;

    fn headlines_in_item_subtree(arena: &org_element::data::NodeArena, item_id: NodeId) -> usize {
        let mut count = 0;
        for &child in &arena[item_id].children {
            if matches!(arena[child].data, Syntax::Headline(_)) {
                count += 1;
            }
            count += headlines_in_item_subtree(arena, child);
        }
        count
    }

    /// A sub-headline following a list item (blank line in between) must
    /// not be swallowed into the item's content, and must appear as a
    /// direct child of the parent headline rather than being lost.
    #[test]
    fn sub_headline_after_blank_not_in_item_and_is_sibling() {
        let input = "* Top Level\n- list item\n\n** Sub-headline\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let top_hl = arena[root].children[0];
        let section = arena[top_hl]
            .children
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::Section))
            .copied()
            .expect("expected Section child of headline");
        let list = arena[section]
            .children
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::PlainList(_)))
            .copied()
            .expect("expected PlainList inside section");
        let item = arena[list].children[0];
        let headlines_in_item = headlines_in_item_subtree(&arena, item);
        assert_eq!(
            headlines_in_item, 0,
            "** Sub-headline must not appear inside the list item (found {} headline(s))",
            headlines_in_item
        );
        let sub_hl_count = arena[top_hl]
            .children
            .iter()
            .filter(|&&n| matches!(arena[n].data, Syntax::Headline(_)))
            .count();
        assert_eq!(
            sub_hl_count, 1,
            "** Sub-headline must be a direct child of * Top Level (found {} headline sibling(s))",
            sub_hl_count
        );
    }

    /// Multiple items — headline follows the last one.
    #[test]
    fn headline_after_multiple_items_not_in_last_item() {
        let input = "* Parent\n- alpha\n- beta\n\n** Child\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let top_hl = arena[root].children[0];
        let section = arena[top_hl]
            .children
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::Section))
            .copied()
            .expect("expected Section");
        let list = arena[section]
            .children
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::PlainList(_)))
            .copied()
            .expect("expected PlainList");

        for &item in &arena[list].children {
            let hl_count = headlines_in_item_subtree(&arena, item);
            assert_eq!(
                hl_count, 0,
                "** Child must not appear inside any list item (item {:?} has {} headline(s))",
                arena[item].location, hl_count
            );
        }
    }

    /// A list at top level (no parent headline) followed by a headline.
    #[test]
    fn top_level_list_followed_by_headline() {
        let input = "- top item\n\n* Headline\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        // Find the PlainList (may be inside a section or directly under root)
        fn find_list(arena: &org_element::data::NodeArena, id: NodeId) -> Option<NodeId> {
            if matches!(arena[id].data, Syntax::PlainList(_)) {
                return Some(id);
            }
            for &c in &arena[id].children {
                if let Some(l) = find_list(arena, c) {
                    return Some(l);
                }
            }
            None
        }
        let list = find_list(&arena, root).expect("expected a PlainList");
        for &item in &arena[list].children {
            let hl_count = headlines_in_item_subtree(&arena, item);
            assert_eq!(
                hl_count, 0,
                "* Headline must not appear inside the list item (found {} headline(s))",
                hl_count
            );
        }
    }
}

/// For description list items (`- TAG :: content`), the paragraph inside
/// the item must start after the ` :: ` separator, not at the tag.
///
/// Emacs org-element sets `:contents-begin` (and thus the paragraph
/// `:begin`) to the first character of the description, not the tag.
/// These tests check the paragraph's `location.start` directly.
mod description_item_paragraph_start {
    use super::*;

    fn para_start(input: &str) -> usize {
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        // Walk: root → section → plain_list → item → paragraph
        let section = arena[root].children[0];
        let list = arena[section].children[0];
        let item = arena[list].children[0];
        let para = arena[item].children[0];
        assert_eq!(
            SyntaxT::from(&arena[para].data),
            SyntaxT::Paragraph,
            "expected Paragraph as first child of item"
        );
        arena[para].location.start
    }

    #[test]
    fn para_start_cases() {
        for (input, expected, desc) in &[
            //   0123456789
            ("- tag :: content\n", 9_usize, "basic ' :: ' separator"),
            (
                "- A :: B :: C\n",
                12,
                "greedy: last ' :: ' is the separator",
            ),
            ("- tag ::\tcontent\n", 9, "tab after '::'"),
            ("- tag ::\n  content\n", 9, "content on next line"),
        ] {
            assert_eq!(para_start(input), *expected, "{}", desc);
        }
    }
}

/// Continuation lines inside a single list item must not create
/// extra Paragraph nodes.  Emacs produces one Paragraph per item.
#[test]
fn no_extra_paragraphs_in_item() {
    // Indented `:key:` lines are continuation lines of the same item.
    let input = "- item\n  :one: first\n  :two: second\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let item_count = count_type(&arena, root, SyntaxT::Item);
    let para_count = count_type(&arena, root, SyntaxT::Paragraph);
    assert_eq!(
        para_count, item_count,
        "Paragraph count ({}) should equal Item count ({}), got {}",
        item_count, para_count, para_count
    );
}
