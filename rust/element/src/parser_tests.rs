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

use crate::data::{NodeArena, NodeId, Syntax, SyntaxT};
use crate::environment::DefaultEnvironment;
use crate::parser::{ParseGranularity, Parser};
use bumpalo::Bump;

fn count_type(arena: &NodeArena, id: NodeId, typ: SyntaxT) -> usize {
    let mut count = 0;
    if SyntaxT::from(&arena[id].data) == typ {
        count += 1;
    }
    for &child in &arena[id].children {
        count += count_type(arena, child, typ);
    }
    count
}

fn get_type_count(input: &str, typ: SyntaxT, granularity: ParseGranularity) -> usize {
    let bump = Bump::new();
    let mut parser = Parser::new(input, granularity, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    count_type(&arena, root, typ)
}

mod plain_list {
    use super::*;

    #[test]
    fn plain_list_kinds() {
        for (input, desc) in &[
            ("- item 1\n- item 2\n- item 3\n", "unordered 3-item"),
            ("- item 1\n- item 2\n", "unordered 2-item"),
            ("1. item 1\n2. item 2\n3. item 3\n", "ordered"),
            (
                "- term 1 :: description 1\n- term 2 :: description 2\n",
                "descriptive",
            ),
            (
                "- [X] checked item\n- [ ] unchecked item\n- [-] partially checked\n",
                "with checkboxes",
            ),
        ] {
            let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
            assert_eq!(
                count, 1,
                "Expected 1 plain list ({}), found {}",
                desc, count
            );
        }
    }

    #[test]
    fn list_empty() {
        let input = "- \n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 plain list for empty item, found {}",
            count
        );
    }

    #[test]
    fn numbered_list_over_ten() {
        let input = "10. item\n11. another\n12. yet another\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let count = count_type(&arena, root, SyntaxT::PlainList);
        let item_count = count_type(&arena, root, SyntaxT::Item);
        assert_eq!(
            count, 1,
            "Expected 1 plain list with items >=10, found {}",
            count
        );
        assert!(
            item_count >= 3,
            "Expected at least 3 items for list items >=10, found {}",
            item_count
        );
    }

    /// List with sub-items followed by more top-level items must not
    /// grow the wrong kind of nesting.  Regression test for the
    /// `lanceberge_aws.org` corpus file where `plain_list_parser` broke
    /// at sub-items and abandoned remaining same-indent siblings,
    /// causing `parse_elements` to re-discover them as a nested list
    /// inside the sub-list rather than as siblings of the parent item.
    ///
    /// Correct structure (2 PlainLists):
    ///   (plain-list    ← outer
    ///     (item "a")
    ///     (item "b"
    ///       (plain-list  ← sub-list inside item b
    ///         (item "c")))
    ///     (item "d"))      ← sibling of a/b, NOT nested
    #[test]
    fn list_with_subitem_after_more_top_items() {
        let input = "- a\n- b\n  - c\n- d\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 2,
            "Expected 2 PlainLists (outer + sub-list for c), found {}",
            count
        );
    }

    /// Two or more blank lines between list items should split them
    /// into separate PlainLists, matching Emacs behavior.
    ///
    /// Correct structure (2 PlainLists):
    ///   (plain-list         ← first list
    ///     (item "item 1")
    ///     (item "item 2"))
    ///   (plain-list         ← second list, separated by blank lines
    ///     (item "item 3"))
    #[test]
    fn blank_lines_split_plain_list() {
        let input = "- item 1\n- item 2\n\n\n- item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 2,
            "Expected 2 PlainLists (split by blank lines), found {}",
            count
        );
    }

    /// List followed by a headline must not nest the headline or remaining
    /// top-level items inside a sub-list.
    #[test]
    fn list_with_subitems_followed_by_headline() {
        let input = "- item 1\n  - sub\n- item 2\n\n** Headline\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 2,
            "Expected 2 PlainLists (outer + sub-list for sub), found {}",
            count
        );
    }

    /// Items after a sub-item at the same indentation as the outer list
    /// must be siblings of their parent item, not nested.
    #[test]
    fn list_with_subitem_in_middle_preserves_top_level() {
        let input = "- a\n  - b\n- c\n  - d\n- e\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 3,
            "Expected 3 PlainLists (outer + sub for b + sub for d), found {}",
            count
        );
    }

    /// A SrcBlock inside a list item must be recognized even when its
    /// body lines are indented *less* than the item's bullet.
    /// `list_struct` terminates the list at such lines, but the item's
    /// content must extend past them so the block parser can find the
    /// matching `#+END_SRC`.
    #[test]
    fn src_block_inside_item() {
        let input = "\
- item

    #+BEGIN_SRC sh
body line
    #+END_SRC
";
        let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 SrcBlock inside a list item with less-indented body, found {}",
            count
        );
    }

    /// A SrcBlock inside a list item followed by more top-level items.
    /// This is the exact pattern from `lanceberge_jq.org`.
    #[test]
    fn src_block_inside_item_with_more_items() {
        let input = "\
- keys

    #+BEGIN_SRC sh
jq '.fruit | keys' fruit.json
    #+END_SRC

- min, max
";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let count = count_type(&arena, root, SyntaxT::SrcBlock);
        assert_eq!(
            count, 1,
            "Expected 1 SrcBlock inside a list item, found {}",
            count
        );
        let item_count = count_type(&arena, root, SyntaxT::Item);
        assert_eq!(
            item_count, 2,
            "Expected exactly 2 items, found {}",
            item_count
        );
    }

    /// After a blank line closes a nested list, Emacs places a
    /// SrcBlock as a direct Section child. Rust keeps it nested
    /// inside the last list item.
    fn dump_tree(arena: &NodeArena, root: NodeId, label: &str) {
        eprintln!("=== TREE: {label} ===");
        let mut stack: Vec<(NodeId, usize)> = vec![(root, 0)];
        while let Some((id, depth)) = stack.pop() {
            let node = &arena[id];
            let t = format!("{:?}", SyntaxT::from(&node.data));
            let loc = node.location;
            let content = node
                .content_location
                .map(|c| format!("({},{})", c.start, c.end))
                .unwrap_or_default();
            let indent = "  ".repeat(depth);
            eprintln!(
                "{indent}{t} ({},{}) content=[{content}]",
                loc.start, loc.end
            );
            for &child in node.children.iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        eprintln!("======================");
    }

    #[test]
    fn src_block_after_blank_line_after_nested_list() {
        let input = r#"- item 1
  - item 2
    - item 3

#+BEGIN_SRC emacs-lisp
(code)
#+END_SRC
"#;
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        dump_tree(&arena, root, "nested-list");

        let sb_count = count_type(&arena, root, SyntaxT::SrcBlock);
        assert_eq!(
            sb_count, 1,
            "Expected exactly one SrcBlock; got {}",
            sb_count
        );

        let root_kids = &arena[root].children;
        let section = root_kids.first().expect("Expected Section");
        let section_kids = &arena[*section].children;
        for &id in section_kids {
            if matches!(arena[id].data, Syntax::SrcBlock(_)) {
                return;
            }
        }
        panic!(
            "SrcBlock must be a direct child of Section, found only: {:?}",
            section_kids
                .iter()
                .map(|id| format!("{:?}", SyntaxT::from(&arena[*id].data)))
                .collect::<Vec<_>>()
        );
    }

    /// Fixed-width lines indented inside a list item body should be
    /// parsed as FixedWidth children of the Item, not as Paragraph.
    #[test]
    fn fixed_width_inside_plain_list_item() {
        let input = r#"- item with fixed width:
   : fixed width content

- next item
"#;
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let mut found_inside_item = false;
        fn walk_for_fw(arena: &NodeArena, id: NodeId, inside_item: bool, found: &mut bool) {
            if *found {
                return;
            }
            if matches!(arena[id].data, Syntax::FixedWidth(_)) && inside_item {
                *found = true;
                return;
            }
            let is_item = matches!(arena[id].data, Syntax::Item(_));
            for &c in &arena[id].children {
                walk_for_fw(arena, c, inside_item || is_item, found);
            }
        }
        walk_for_fw(&arena, root, false, &mut found_inside_item);
        assert!(
            found_inside_item,
            "FixedWidth must be a child of an Item node, not a direct Section child"
        );
    }

    /// Emacs `org-current-text-column` uses `string-width` to measure
    /// leading whitespace — a TAB always counts as 8 cells regardless of
    /// the current column position.  Our `get_indent` uses screen-column
    /// math (`\t` advances to next multiple of 8), which gives a phantom
    /// gap for mixed space+tab indentation.
    ///
    /// For a line like `   \t* text` (3 spaces + tab + `* `):
    ///   - Emacs indent = string-width("   \t") = 3 + 8 = 11
    ///   - Rust  indent = screen-column after "   \t" = 8
    ///
    /// This causes continuation lines at column 10 (e.g. `\t  (text)`)
    /// to be treated as continuations in Rust (`10 > 8`) but as
    /// list-terminating in Emacs (`10 <= 11`).
    #[test]
    fn tab_with_spaces_indent_mismatch_with_emacs() {
        // Two items, each followed by a blank line + continuation.
        // Emacs: 2 PlainLists (continuation terminates each list).
        // Rust:  1 PlainList (continuation indent=10 > get_indent=8 → stays in list).
        let input = "   \t* item1\n\n\t  (cont1)\n\n   \t* item2\n\n\t  (cont2)\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        // Emacs produces 2 PlainLists (each star-item is a separate list).
        let pl = count_type(&arena, root, SyntaxT::PlainList);
        assert_eq!(
            pl, 2,
            "Expected 2 PlainLists (Emacs: continuation at col 10 <= Emacs-indent(11) \
             terminates list), got {pl}"
        );

        // Emacs produces 3 Paragraphs: one inside each item + the two
        // continuations at section level = 4 paragraphs total (2 in items, 2 at section).
        // Actually Emacs: item1[1 para], cont1[1 para @section], item2[1 para], cont2[1 para @section]
        let para = count_type(&arena, root, SyntaxT::Paragraph);
        assert_eq!(
            para, 4,
            "Expected 4 Paragraphs (2 inside items, 2 at section level), got {para}"
        );
    }

    /// Same string-width vs screen-column discrepancy affects
    /// `item_content_end`: a continuation line after a blank line
    /// whose `string-width` indent ≤ item indent should terminate
    /// the item content (matching Emacs).
    #[test]
    fn single_item_terminated_by_tab_indented_continuation() {
        // Single item with blank line + continuation.
        // Emacs: Paragraph at section level (continuation NOT inside item).
        // Rust:  Paragraph inside item (indent 10 > get_indent 8 → continuation).
        let input = "   \t* item1\n\n\t  (cont1)\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        // Count paragraphs inside items vs at section level.
        let section = arena[root].children[0];
        let section_paras: Vec<_> = arena[section]
            .children
            .iter()
            .filter(|&&c| matches!(arena[c].data, Syntax::Paragraph))
            .collect();
        assert_eq!(
            section_paras.len(),
            1,
            "Expected 1 Paragraph at section level (the continuation), got {}",
            section_paras.len()
        );

        // Items should contain exactly one paragraph each (only "item1").
        let items: Vec<NodeId> = arena[section]
            .children
            .iter()
            .filter(|&&c| matches!(arena[c].data, Syntax::PlainList(_)))
            .copied()
            .collect();
        if let Some(list) = items.first() {
            // Each item has exactly 1 paragraph (item text, not continuation)
            for &item in &arena[*list].children {
                let ip = count_type(&arena, item, SyntaxT::Paragraph);
                assert_eq!(
                    ip, 1,
                    "Item should have exactly 1 Paragraph (the item text), got {ip}"
                );
            }
        }
    }

    #[test]
    fn tab_indented_nested_star_list() {
        // Tab-indented `\t * :term VALUE` inside a `-` outer list.
        // The after_bullet calculation must use byte offset, not
        // screen-width indent (tabs expand to 8 in indent calc but
        // are only 1 byte).  Bug: item.indent is screen-width (9 for
        // `\t `), so after_bullet = pos + 9 + 1 + 1 = pos+11 instead
        // of pos+4, pointing to mid-word instead of `:term VALUE`.
        let input = "- outer\n\t * :term VALUE\n\t   内容行\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        // Should have 2 PlainLists (outer + nested)
        let pl = count_type(&arena, root, SyntaxT::PlainList);
        assert_eq!(pl, 2, "Expected 2 PlainLists, got {pl}");
        // Inner paragraph should start at the tag content, not mid-word
        let mut inner_para_start = 0;
        fn find_inner_para(arena: &NodeArena, id: NodeId, target: &mut usize) {
            if SyntaxT::from(&arena[id].data) == SyntaxT::Paragraph
                && arena[id].location.start > 10
            {
                *target = arena[id].location.start;
            }
            for &c in &arena[id].children {
                find_inner_para(arena, c, target);
            }
        }
        find_inner_para(&arena, root, &mut inner_para_start);
        assert!(
            inner_para_start > 0,
            "Should find an inner paragraph"
        );
        // Expected: content starts at byte 12 (`:term VALUE`)
        // Bug puts it at byte 19 (`A` in `VALUE`)
        assert_eq!(
            inner_para_start, 12,
            "Inner paragraph should start at byte 12 (:term VALUE), got {inner_para_start}"
        );
    }
}

mod item {
    use super::*;

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

        fn headlines_in_item_subtree(arena: &crate::data::NodeArena, item_id: NodeId) -> usize {
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
            let mut parser =
                Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
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
            let mut parser =
                Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
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
            let mut parser =
                Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
            let (arena, root) = parser.parse_buffer();

            // Find the PlainList (may be inside a section or directly under root)
            fn find_list(arena: &crate::data::NodeArena, id: NodeId) -> Option<NodeId> {
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
            let mut parser =
                Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
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
}

mod headline {
    use super::*;

    #[test]
    fn basic_headline() {
        for (input, desc) in &[
            ("* \n", "minimal"),
            ("****  \n", "four stars"),
            ("* title\n", "with title"),
            ("* TODO [#A] title\n", "with priority"),
            ("* title :tag1:tag2:\n", "with tags"),
            ("* TODO title\n", "with keyword"),
        ] {
            let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 headline ({}), found {}", desc, count);
        }
    }

    #[test]
    fn multiple_headlines() {
        let input = "* Level 1\n** Level 2\n*** Level 3\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 3, "Expected 3 headlines, found {}", count);
    }

    #[test]
    fn headline_with_tab_after_stars() {
        let input = "*\tOne\n**\tSub\n*\tTwo\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let h1 = root_children.first().expect("Expected first headline");
        let Syntax::Headline(_) = &arena[*h1].data else {
            panic!("Expected Headline for h1, got {:?}", arena[*h1].data);
        };
        let h1_end = arena[*h1].location.end;
        let h2 = root_children
            .get(1)
            .expect("Expected second top-level headline");
        let Syntax::Headline(_) = &arena[*h2].data else {
            panic!("Expected Headline for h2, got {:?}", arena[*h2].data);
        };
        assert!(
            h1_end <= arena[*h2].location.start,
            "First headline end ({}) should not overlap second headline start ({}): \
             headlines with tab should form proper subtree boundaries",
            h1_end,
            arena[*h2].location.start
        );
    }

    #[test]
    fn headline_with_unicode_tags() {
        let input = "* title :café:标签:\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let h = root_children.first().expect("Expected headline");
        let Syntax::Headline(data) = &arena[*h].data else {
            panic!("Expected Headline");
        };
        let tag_strs: Vec<&str> = data.tags.iter().map(|t| t.0).collect();
        assert!(
            tag_strs.contains(&"café"),
            "Tags should include unicode 'café', got {:?}",
            tag_strs
        );
        assert!(
            tag_strs.contains(&"标签"),
            "Tags should include CJK '标签', got {:?}",
            tag_strs
        );
    }

    #[test]
    fn headline_comment_not_in_title() {
        let input = "* COMMENT stuff\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let h = root_children.first().expect("Expected headline");
        let Syntax::Headline(data) = &arena[*h].data else {
            panic!("Expected Headline");
        };
        assert!(
            !data.title.starts_with("COMMENT"),
            "Title should not include COMMENT keyword, got: {:?}",
            data.title
        );
    }

    /// Headline title text is a secondary string in Emacs org-element and is
    /// stored in the `:title` property, never in `org-element-contents`.
    /// Therefore, parsed title objects (PlainText, Bold, Link, …) must NOT
    /// appear as direct children of the Headline node.
    ///
    /// Headline children should only be Section and sub-Headline nodes.
    mod title_objects_not_in_children {
        use super::*;

        fn headline_direct_child_types(input: &str) -> Vec<SyntaxT> {
            let bump = Bump::new();
            let mut parser =
                Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
            let (arena, root) = parser.parse_buffer();
            // root → headline (first child)
            let hl = *arena[root].children.first().expect("expected a headline");
            assert_eq!(SyntaxT::from(&arena[hl].data), SyntaxT::Headline);
            arena[hl]
                .children
                .iter()
                .map(|&id| SyntaxT::from(&arena[id].data))
                .collect()
        }

        /// Title objects must not appear as direct Headline children regardless
        /// of what markup the title contains.
        #[test]
        fn title_objects_not_direct_children() {
            let inline_types = [
                SyntaxT::PlainText,
                SyntaxT::Bold,
                SyntaxT::Italic,
                SyntaxT::Underline,
                SyntaxT::Code,
                SyntaxT::Verbatim,
                SyntaxT::Link,
                SyntaxT::StrikeThrough,
            ];
            for (input, desc) in [
                ("* foo bar\n", "plain text title"),
                ("* foo *bold* bar\n", "title with markup"),
                ("* See [[https://example.com][text]]\n", "title with link"),
                (
                    "* Title with *bold*\nsome body text\n** Sub\n",
                    "headline with body and sub",
                ),
            ] {
                let types = headline_direct_child_types(input);
                for t in inline_types {
                    assert!(
                        !types.contains(&t),
                        "{:?} must not be a direct child of Headline ({}); got {:?}",
                        t,
                        desc,
                        types
                    );
                }
            }
        }
    }
}

mod table {
    use super::*;

    #[test]
    fn simple_table() {
        for (input, desc) in &[
            (
                "| col 1 | col 2 |\n|-----|-----|\n| a | b |\n| c | d |\n",
                "rows + hline",
            ),
            ("| a | b |\n| c | d |\n", "data only"),
            (
                "| head1 | head2 |\n|--------|--------|\n| body1 | body2 |\n",
                "header + hline",
            ),
        ] {
            let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 table ({}), found {}", desc, count);
        }
    }

    #[test]
    fn table_row_count() {
        let input = "| a | b |\n| c | d |\n| e | f |\n";
        let count = get_type_count(input, SyntaxT::TableRow, ParseGranularity::Element);
        assert_eq!(count, 3, "Expected 3 table rows, found {}", count);
    }

    #[test]
    fn table_variants() {
        for (input, desc) in &[
            ("  | a | b |\n  | c | d |\n", "indented"),
            ("| | b |\n| c | |\n", "empty cells"),
            ("| a | b |", "no trailing newline"),
        ] {
            let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 table ({}), found {}", desc, count);
        }
    }

    #[test]
    fn table_hline_detection() {
        let input = "| h1 | h2 |\n|---|\n| c1 | c2 |\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let table = section_children.first().expect("Expected table");
        let table_children = &arena[*table].children;

        let rule_row = table_children
            .get(1)
            .expect("Expected second table row (hline)");
        let Syntax::TableRow(row_type) = &arena[*rule_row].data else {
            panic!("Expected TableRow, got: {:?}", arena[*rule_row].data);
        };
        assert!(
            matches!(row_type, crate::table::TableRowType::Rule),
            "Expected hline row to be Rule, got {:?}",
            row_type
        );
    }

    #[test]
    fn table_blank_line_terminates() {
        let input = "| a |\n\nend\n";
        let count = get_type_count(input, SyntaxT::TableRow, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 table row (blank line should terminate table), found {}",
            count
        );
    }

    #[test]
    fn table_row_children_are_cells() {
        // "| foo | bar | baz |\n" — 3 columns → 3 TableCell children of the row.
        // Emacs wraps each |…| segment in a table-cell element; the parser must
        // not place raw objects (PlainText, Link, …) directly under table-row.
        let input = "| foo | bar | baz |\n";
        let cell_count = get_type_count(input, SyntaxT::TableCell, ParseGranularity::Object);
        assert_eq!(
            cell_count, 3,
            "expected 3 TableCell nodes, got {}",
            cell_count
        );
    }

    #[test]
    fn table_cell_plain_text_not_direct_child_of_row() {
        // Plain-text inside a table cell must live under the TableCell, not the
        // TableRow.  Rust currently skips the TableCell wrapper and puts objects
        // directly in the row.
        let input = "| foo | bar |\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        fn find_first(
            arena: &crate::data::NodeArena<'_, '_>,
            id: NodeId,
            typ: SyntaxT,
        ) -> Option<NodeId> {
            if SyntaxT::from(&arena[id].data) == typ {
                return Some(id);
            }
            for &child in &arena[id].children {
                if let Some(found) = find_first(arena, child, typ) {
                    return Some(found);
                }
            }
            None
        }

        let row = find_first(&arena, root, SyntaxT::TableRow).expect("Expected a TableRow");
        let row_children = &arena[row].children;
        assert!(
            row_children
                .iter()
                .all(|&child| matches!(&arena[child].data, Syntax::TableCell)),
            "All children of TableRow must be TableCell, got {:?}",
            row_children
                .iter()
                .map(|&c| SyntaxT::from(&arena[c].data))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn tblfm_line_is_absorbed_into_table() {
        let input = "| a | b |\n| c | d |\n#+TBLFM: $2=$1*2\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;

        let table_count = section_children
            .iter()
            .filter(|&&id| matches!(&arena[id].data, Syntax::Table))
            .count();
        let spreadsheet_count = section_children
            .iter()
            .filter(|&&id| matches!(&arena[id].data, Syntax::Spreadsheet(_)))
            .count();
        let keyword_count = section_children
            .iter()
            .filter(|&&id| matches!(&arena[id].data, Syntax::Keyword(_)))
            .count();

        assert_eq!(
            table_count, 1,
            "Expected 1 Table node (Emacs absorbs #+TBLFM: into Table), found {} Spreadsheet, {} Keyword",
            spreadsheet_count, keyword_count
        );
    }

    #[test]
    fn results_preceded_by_blank_line_absorbed_into_table() {
        let input = "#+END_SRC\n\n#+RESULTS:\n| a | b |\n| c | d |\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        assert!(find_first(&arena, root, SyntaxT::Table).is_some());
        assert!(find_first(&arena, root, SyntaxT::Keyword).is_none());
    }

    #[test]
    fn table_with_tblfm_absorbed_no_results_prefix() {
        let input =
            " =code= simple case\n| 13:59 | 839 |\n| 13:59:05 | 50345 |\n#+TBLFM: $2=formula($1)\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        assert!(find_first(&arena, root, SyntaxT::Table).is_some());
        assert!(find_first(&arena, root, SyntaxT::Keyword).is_none());
    }

    #[test]
    fn non_table_line_not_absorbed_after_results_table() {
        let input = "#+RESULTS:\n| a | b |\n|---+---|\nnot a table row\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        assert!(find_first(&arena, root, SyntaxT::Table).is_some());
        assert!(find_first(&arena, root, SyntaxT::Paragraph).is_some());
    }

    #[test]
    fn mid_line_pipe_not_treated_as_table_row() {
        let input = "#+RESULTS:\n| a | b |\n|---+---|\nconfig.el: (foo | bar)\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let table = find_first(&arena, root, SyntaxT::Table).unwrap();
        let table_children = &arena[table].children;
        // Only the 2 rows, not the config.el line
        assert_eq!(
            table_children.len(),
            2,
            "Table should contain exactly 2 rows, not the mid-line pipe line"
        );
    }

    fn find_first(arena: &NodeArena, id: NodeId, target: SyntaxT) -> Option<NodeId> {
        if SyntaxT::from(&arena[id].data) == target {
            return Some(id);
        }
        for &child in &arena[id].children {
            if let found @ Some(_) = find_first(arena, child, target) {
                return found;
            }
        }
        None
    }
}

mod blocks {
    use super::*;

    #[test]
    fn block_types() {
        for (input, typ, desc) in &[
            (
                "#+BEGIN_CENTER\nCentered text\n#+END_CENTER\n",
                SyntaxT::CenterBlock,
                "center",
            ),
            (
                "#+BEGIN_VERSE\nVerse line 1\nVerse line 2\n#+END_VERSE\n",
                SyntaxT::VerseBlock,
                "verse",
            ),
            (
                "#+BEGIN_SPECIAL\nCustom block content\n#+END_SPECIAL\n",
                SyntaxT::SpecialBlock,
                "special",
            ),
            (
                "#+BEGIN: my-block :param value\nBlock content\n#+END:\n",
                SyntaxT::DynamicBlock,
                "dynamic",
            ),
            (
                "#+begin_center\nCentered\n#+end_center\n",
                SyntaxT::CenterBlock,
                "center lowercase",
            ),
            (
                "#+BEGIN_CENTER\ncenter\n#+end_center \n",
                SyntaxT::CenterBlock,
                "center trailing space on END",
            ),
        ] {
            let count = get_type_count(input, *typ, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 {} block, found {}", desc, count);
        }
    }

    #[test]
    fn incomplete_block() {
        let input = "#+BEGIN_CENTER\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 center blocks for incomplete, found {}",
            count
        );
    }

    #[test]
    fn export_block_type() {
        let input = "#+BEGIN_EXPORT html\n<div>HTML</div>\n#+END_EXPORT\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let block = section_children.first().expect("Expected export block");

        let Syntax::ExportBlock(data) = &arena[*block].data else {
            panic!("Expected ExportBlock, got: {:?}", arena[*block].data);
        };
        assert_eq!(
            data.type_s, "html",
            "Expected export type_s 'html', got {:?}",
            data.type_s
        );
    }

    #[test]
    fn src_block_language() {
        let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let block = section_children.first().expect("Expected src block");

        let Syntax::SrcBlock(data) = &arena[*block].data else {
            panic!("Expected SrcBlock, got: {:?}", arena[*block].data);
        };
        assert_eq!(
            data.language,
            Some("python"),
            "Expected language 'python', got {:?}",
            data.language
        );
    }
}

mod drawer {
    use super::*;

    #[test]
    fn drawers_simple_and_case_insensitive() {
        for (input, desc) in &[
            (":TEST:\nDrawer content\n:END:\n", "basic"),
            (":test:\nDrawer content\n:end:\n", "case insensitive"),
        ] {
            let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 drawer ({}), found {}", desc, count);
        }
    }

    #[test]
    fn property_drawer_and_node_properties() {
        for (input, expected_props, desc) in &[
            (
                ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n",
                1_usize,
                "single property",
            ),
            (
                ":PROPERTIES:\n:ID: test-id\n:CUSTOM_ID: custom-id\n:END:\n",
                2,
                "two properties",
            ),
        ] {
            let pd_count =
                get_type_count(input, SyntaxT::PropertyDrawer, ParseGranularity::Element);
            assert_eq!(
                pd_count, 1,
                "Expected 1 property drawer ({}), found {}",
                desc, pd_count
            );
            let np_count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
            assert_eq!(
                np_count, *expected_props,
                "Expected {} node properties ({}), found {}",
                expected_props, desc, np_count
            );
        }
    }

    #[test]
    fn incomplete_drawer() {
        let input = ":TEST:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 drawers for incomplete, found {}",
            count
        );
    }

    /// Emacs parses `:PROPERTIES:` drawers that appear right after a
    /// headline body (before the blank line that ends the section's
    /// first paragraph). Rust absorbs the drawer into a Paragraph
    /// instead of creating a PropertyDrawer node.
    #[test]
    fn property_drawer_after_headline_no_blank_line() {
        let input = "** my-comment-box\n\
                      :PROPERTIES:\n\
                      :CREATED:  [2019-01-11 Fri 14:36]\n\
                      :END:\n\n\
                      paragraph after blank line\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let pd_count = count_type(&arena, root, SyntaxT::PropertyDrawer);
        assert!(
            pd_count > 0,
            "Rust absorbs ':' lines after headline body into Paragraph instead of PropertyDrawer"
        );
    }

    /// A regular drawer placed immediately after a property drawer must
    /// have its body parsed into a Paragraph. Regression from
    /// `karl_voit_config.org` (`:LINKS:` drawer): Rust recognises the
    /// drawer but produces no Paragraph child for its contents.
    #[test]
    fn paragraph_inside_drawer() {
        let input = "* H\n\
                     :PROPERTIES:\n\
                     :ID:       x\n\
                     :END:\n\
                     :LINKS:\n\
                     [2026-02-16 Mon 19:10] <- [[id:foo][General]]\n\
                     :END:\n\n\
                     text\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        fn drawer_has_para(arena: &NodeArena, id: NodeId, in_drawer: bool) -> bool {
            if in_drawer && matches!(arena[id].data, Syntax::Paragraph) {
                return true;
            }
            let is_drawer = matches!(arena[id].data, Syntax::Drawer(_));
            arena[id]
                .children
                .iter()
                .any(|&c| drawer_has_para(arena, c, in_drawer || is_drawer))
        }
        assert!(
            drawer_has_para(&arena, root, false),
            "Expected a Paragraph inside the :LINKS: drawer; Rust produced none"
        );
    }
}

mod planning {
    use super::*;

    #[test]
    fn planning_all_timestamps() {
        for (input, desc) in &[
            ("* TODO Task\nDEADLINE: <2023-12-31>\n", "deadline"),
            ("* TODO Task\nSCHEDULED: <2023-12-31>\n", "scheduled"),
            ("* TODO Task\nCLOSED: [2023-12-31]\n", "closed"),
            (
                "* TODO Task\nDEADLINE: <2023-12-31> SCHEDULED: <2023-12-30> CLOSED: [2023-12-29]\n",
                "all timestamps",
            ),
        ] {
            let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 planning ({}), found {}", desc, count);
        }
    }
}

mod comment {
    use super::*;

    #[test]
    fn basic_comment() {
        for (input, desc) in &[
            ("# Comment\n", "single line"),
            ("# Line 1\n# Line 2\n# Line 3\n", "multi-line"),
            ("  # Indented comment\n", "indented"),
        ] {
            let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 comment ({}), found {}", desc, count);
        }
    }
}

mod clock {
    use super::*;

    #[test]
    fn clock_variants() {
        for (input, desc) in &[
            ("CLOCK: [2023-10-13 Fri 14:40]\n", "running"),
            (
                "CLOCK: [2023-10-13 Fri 14:40]--[2023-10-13 Fri 14:51] => 0:11\n",
                "closed",
            ),
            ("Clock: [2023-10-13 Fri 14:40]\n", "case insensitive"),
            ("CLOCK: => 0:11\n", "duration only"),
        ] {
            let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 clock ({}), found {}", desc, count);
        }
    }

    #[test]
    fn must_be_bol() {
        for (input, desc) in &[
            ("   CLOCK: [2023-10-13 Fri 14:40]\n", "leading spaces"),
            ("\tCLOCK: [2023-10-13 Fri 14:40]\n", "leading tab"),
        ] {
            let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 clock with {}, found {}", desc, count);
        }
    }

    #[test]
    fn invalid_date() {
        let input = "CLOCK: [2023-02-29 Wed 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 clock (Feb 29 in non-leap year), found {}",
            count
        );
    }
}

mod diary_sexp {
    use super::*;

    #[test]
    fn diary_sexp() {
        for (input, desc) in &[
            (
                "%%(org-anniversary 1956 5 14) Arthur Dent is %d years old\n",
                "full",
            ),
            ("%%(diary-sexp)\n", "minimal"),
        ] {
            let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
            assert_eq!(
                count, 1,
                "Expected 1 diary sexp ({}), found {}",
                desc, count
            );
        }
    }

    #[test]
    fn must_be_bol() {
        let input = " %(diary-sexp)\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 diary sexp (not at BOL), found {}",
            count
        );
    }
}

mod latex_environment {
    use super::*;

    #[test]
    fn latex_environment() {
        for (input, desc) in &[
            (
                "\\begin{equation}\nE = mc^2\n\\end{equation}\n",
                "basic equation",
            ),
            (
                "\\begin{equation}\n\\frac{a}{b}\n\\end{equation}\n",
                "equation with frac",
            ),
            (
                "\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}\n",
                "aligned",
            ),
        ] {
            let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
            assert_eq!(
                count, 1,
                "Expected 1 latex environment ({}), found {}",
                desc, count
            );
        }
    }
}

mod footnote_definition {
    use super::*;

    #[test]
    fn footnote_definition() {
        for (input, desc) in &[
            ("[fn:1] This is a footnote.\n", "numbered"),
            ("[fn:my-note] This is a named footnote.\n", "named"),
            ("[fn:1]\nMulti-line\nfootnote content\n", "multiline"),
        ] {
            let count = get_type_count(
                input,
                SyntaxT::FootnoteDefinition,
                ParseGranularity::Element,
            );
            assert_eq!(
                count, 1,
                "Expected 1 footnote definition ({}), found {}",
                desc, count
            );
        }
    }

    #[test]
    fn content_location() {
        let input = "[fn:1] This is footnote content\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected at least one child");
        let section_children = &arena[*section].children;
        let fn_node = section_children
            .first()
            .expect("Expected footnote in section");

        let Syntax::FootnoteDefinition(_) = &arena[*fn_node].data else {
            panic!(
                "Expected FootnoteDefinition, got: {:?}",
                arena[*fn_node].data
            );
        };
        let content_loc = &arena[*fn_node]
            .content_location
            .expect("Footnote definition should have content_location");
        assert_eq!(
            content_loc.start, 7,
            "contents-begin should be after [fn:1] and space"
        );
        let content = &input[content_loc.start..content_loc.end];
        assert_eq!(content, "This is footnote content", "content should match");
    }

    #[test]
    fn with_label() {
        let input = "[fn:my-label] Some content\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected at least one child");
        let section_children = &arena[*section].children;
        let fn_node = section_children
            .first()
            .expect("Expected footnote in section");

        let Syntax::FootnoteDefinition(data) = &arena[*fn_node].data else {
            panic!("Expected FootnoteDefinition syntax type");
        };
        assert_eq!(data.label, "my-label", "Label should be extracted");
        assert_eq!(data.value, "Some content", "Value should be raw text");
    }

    #[test]
    fn footnote_reference_inline() {
        let input = "Paragraph with reference[fn:1] and another[fn:inline:inline note]\n";
        let count = get_type_count(input, SyntaxT::FootnoteReference, ParseGranularity::Object);
        assert_eq!(count, 2, "Expected 2 footnote references, found {}", count);
    }
}

mod fixed_width {
    use super::*;
    use crate::markup::strip_fixed_width_colons;

    #[test]
    fn basic() {
        for (input, desc) in &[
            (": Fixed width line\n", "basic"),
            ("  : Indented fixed width\n", "indented"),
            (
                "#+NAME: my-fixed\n: Fixed width line\n",
                "with name affiliated",
            ),
            (": Line 1\n: Line 2\n: Line 3\n", "multiple lines"),
        ] {
            let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
            assert_eq!(
                count, 1,
                "Expected 1 fixed width ({}), found {}",
                desc, count
            );
        }
    }

    #[test]
    fn multiline_value_strips_colons() {
        let input = ": Line 1\n: Line 2\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let fw_node = section_children.first().expect("Expected fixed-width node");

        let Syntax::FixedWidth(raw) = &arena[*fw_node].data else {
            panic!("Expected FixedWidth, got: {:?}", arena[*fw_node].data);
        };
        let value = strip_fixed_width_colons(raw);
        assert!(
            !value.contains(':'),
            "Fixed-width value should not contain colons, got: {:?}",
            value
        );
        assert!(
            value.contains("Line 1"),
            "Fixed-width value should contain 'Line 1', got: {:?}",
            value
        );
        assert!(
            value.contains("Line 2"),
            "Fixed-width value should contain 'Line 2', got: {:?}",
            value
        );
    }
}

mod babel_call {
    use super::*;

    #[test]
    fn babel_call() {
        for (input, desc) in &[
            ("#+CALL: test()\n", "uppercase"),
            ("#+call: test()\n", "case insensitive"),
            ("#+CALL: test[:results output]()\n", "with header"),
            ("#+CALL: test(n=4)\n", "with arguments"),
        ] {
            let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
            assert_eq!(
                count, 1,
                "Expected 1 babel call ({}), found {}",
                desc, count
            );
        }
    }
}

mod node_properties {
    use super::*;

    #[test]
    fn multiple_node_properties() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:PRIORITY: A\n: tags: :foo:bar:\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert_eq!(count, 3, "Expected 3 node properties, found {}", count);
    }

    /// `:END:` closes the drawer and must not be parsed as a `NodeProperty`
    /// with key `"END"`.  Regression test: the content range previously
    /// included the `:END:` line, causing `parse_node_property_line` to
    /// emit a spurious extra property.
    #[test]
    fn end_marker_not_a_property() {
        let input = ":PROPERTIES:\n:ID: abc123\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            ":END: must not be parsed as a node property (got {count})"
        );
    }
}

mod keyword {
    use super::*;

    #[test]
    fn keyword() {
        for (input, desc) in &[
            ("#+KEYWORD: value\n", "basic"),
            ("#+keyword: value\n", "case insensitive"),
            ("#+KEYWORD:    spaced value\n", "spaced"),
            ("#+KEYWORD: value\nparagraph\n", "with following paragraph"),
        ] {
            let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 keyword ({}), found {}", desc, count);
        }
    }

    #[test]
    fn keyword_edge_cases() {
        let count = get_type_count(
            "#+KEY: val\n#+EMPTY:\n#+COLONS: a::b::c\n#+UNICODE: café 标签\n",
            SyntaxT::Keyword,
            ParseGranularity::Element,
        );
        assert_eq!(count, 4, "Expected 4 keywords, found {}", count);
    }
}

mod affiliated {
    use super::*;

    #[test]
    fn collects_all_affiliated_types() {
        let input = "#+NAME: my-name\n#+CAPTION: my caption\n#+ATTR_HTML: class=foo\n#+HEADER: :var x=1\n: fixed-width\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 fixed-width with affiliated keywords, found {}",
            count
        );
    }
}

mod horizontal_rule {
    use super::*;

    #[test]
    fn horizontal_rule_basic_and_with_spaces() {
        for (input, desc) in &[("-----\n", "basic"), ("   -----   \n", "with spaces")] {
            let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
            assert_eq!(
                count, 1,
                "Expected 1 horizontal rule ({}), found {}",
                desc, count
            );
        }
    }

    #[test]
    fn horizontal_rule_after_para_before_list() {
        // "para\n\n------\n- item\n"
        // Emacs parses: paragraph, horizontal_rule (6 dashes), plain_list
        // Rust should produce the same structure.
        for granularity in &[ParseGranularity::Element, ParseGranularity::Object] {
            let input = "para\n\n------\n- item\n";
            let bump = Bump::new();
            let mut parser = Parser::new(input, *granularity, DefaultEnvironment, &bump);
            let (arena, root) = parser.parse_buffer();
            let hr_count = count_type(&arena, root, SyntaxT::HorizontalRule);
            let list_count = count_type(&arena, root, SyntaxT::PlainList);
            assert_eq!(
                hr_count, 1,
                "Expected 1 horizontal rule before list (granularity={:?}), found {}",
                granularity, hr_count
            );
            assert_eq!(
                list_count, 1,
                "Expected 1 plain list after horizontal rule (granularity={:?}), found {}",
                granularity, list_count
            );
        }
    }

    /// A horizontal rule directly after a non-blank paragraph line (no
    /// intervening blank line) must still break the paragraph and be
    /// recognised. Regression from `karl_voit_config.org`:
    /// `about 24 miles per\n--------\n` — Emacs emits paragraph +
    /// horizontal-rule, but Rust absorbs the dashes into the paragraph.
    #[test]
    fn horizontal_rule_directly_after_paragraph_line() {
        let input = "about 24 miles per\n--------\n\nAnswer\n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 horizontal rule after a non-blank paragraph line, found {}",
            count
        );
    }
}

mod inlinetask {
    use super::*;

    #[test]
    fn fifteen_stars_is_not_headline() {
        let input = "*************** Inlinetask\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let first = root_children.first().expect("Expected first child");
        assert!(
            !matches!(arena[*first].data, Syntax::Headline(_)),
            "15+ stars should NOT be a Headline, got: {:?}",
            arena[*first].data
        );
    }

    /// An inline task closed by a `*************** END` marker spans through
    /// that marker as a SINGLE node, and the body between the markers is
    /// nested inside it. Emacs (with org-inlinetask) parses one inlinetask
    /// containing the src block; the END line is not its own node. Rust used
    /// to emit two inline tasks (one for END) and leave the body as a sibling.
    #[test]
    fn inline_task_with_end_marker_nests_body() {
        let input = "* Top\n\
                     *************** TODO task\n\
                     #+begin_src elisp\n\
                     (foo)\n\
                     #+end_src\n\
                     *************** END\n\
                     after\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let it_count = count_type(&arena, root, SyntaxT::InlineTask);
        assert_eq!(
            it_count, 1,
            "expected exactly 1 InlineTask (END is not a separate task), found {}",
            it_count
        );

        fn src_in_inlinetask(arena: &NodeArena, id: NodeId, inside: bool) -> bool {
            if inside && matches!(arena[id].data, Syntax::SrcBlock(_)) {
                return true;
            }
            let now = inside || matches!(arena[id].data, Syntax::InlineTask(_));
            arena[id]
                .children
                .iter()
                .any(|&c| src_in_inlinetask(arena, c, now))
        }
        assert!(
            src_in_inlinetask(&arena, root, false),
            "expected the src block nested inside the inline task"
        );
    }
}

mod line_break {
    use super::*;

    /// `text \\` at end of a line is an Org line break; the paragraph
    /// continues on the next line. From the Yantar92 corpus (LOGBOOK entries
    /// like `[2019-12-18 Wed 16:27] \\`).
    #[test]
    fn line_break_at_end_of_line() {
        let count = get_type_count(
            "first \\\\\nsecond\n",
            SyntaxT::LineBreak,
            ParseGranularity::Object,
        );
        assert_eq!(count, 1, "expected 1 LineBreak, found {}", count);
    }

    /// A longer backslash run or a mid-line `\\` (followed by non-whitespace)
    /// is not a line break.
    #[test]
    fn non_line_break_backslashes() {
        assert_eq!(
            get_type_count(
                "a\\\\\\\\\nx\n",
                SyntaxT::LineBreak,
                ParseGranularity::Object
            ),
            0,
            "four backslashes are not a line break"
        );
        assert_eq!(
            get_type_count("a \\\\ b\n", SyntaxT::LineBreak, ParseGranularity::Object),
            0,
            "mid-line `\\\\` followed by text is not a line break"
        );
    }
}

mod object_parsing {
    use super::*;

    #[test]
    fn basic_markup() {
        for (input, typ) in &[
            ("*bold*\n", SyntaxT::Bold),
            ("/italic/\n", SyntaxT::Italic),
            ("_underline_\n", SyntaxT::Underline),
            ("+strikethrough+\n", SyntaxT::StrikeThrough),
            ("~code~\n", SyntaxT::Code),
            ("=verbatim=\n", SyntaxT::Verbatim),
        ] {
            let count = get_type_count(input, *typ, ParseGranularity::Object);
            assert_eq!(count, 1, "Expected 1 {:?} object, found {}", typ, count);
        }
    }

    #[test]
    fn single_char_markup() {
        for (input, typ) in &[("*a*\n", SyntaxT::Bold), ("/a/\n", SyntaxT::Italic)] {
            let count = get_type_count(input, *typ, ParseGranularity::Object);
            assert_eq!(
                count,
                1,
                "Expected 1 {:?} from single-char input '{}', found {}",
                typ,
                input.trim(),
                count
            );
        }
    }

    #[test]
    fn other_objects() {
        for (input, typ, desc) in &[
            ("\\alpha\n", SyntaxT::Entity, "entity"),
            ("[[https://example.com][link]]\n", SyntaxT::Link, "link"),
            ("<<target>>\n", SyntaxT::Target, "target"),
            ("<2023-12-31>\n", SyntaxT::Timestamp, "timestamp"),
        ] {
            let count = get_type_count(input, *typ, ParseGranularity::Object);
            assert_eq!(count, 1, "Expected 1 {} object, found {}", desc, count);
        }
    }

    #[test]
    fn objects_after_punctuation() {
        for (input, typ, desc) in &[
            ("[[link]].\n", SyntaxT::Link, "link"),
            ("<<target>>.\n", SyntaxT::Target, "target"),
            ("<2023-12-31>.\n", SyntaxT::Timestamp, "timestamp"),
        ] {
            let count = get_type_count(input, *typ, ParseGranularity::Object);
            assert_eq!(count, 1, "Expected 1 {} before '.', found {}", desc, count);
        }
    }

    #[test]
    fn timestamp_with_time_range() {
        let input = "<2023-12-01 10:15-11:30>\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let para = section_children.first().expect("Expected paragraph");
        let para_children = &arena[*para].children;
        let ts_node = para_children.first().expect("Expected timestamp");

        let Syntax::Timestamp(data) = &arena[*ts_node].data else {
            panic!("Expected Timestamp, got: {:?}", arena[*ts_node].data);
        };
        assert_eq!(
            data.hour_end,
            Some(11),
            "Expected hour_end 11 from time range 10:15-11:30, got {:?}",
            data.hour_end
        );
        assert_eq!(
            data.minute_end,
            Some(30),
            "Expected minute_end 30 from time range 10:15-11:30, got {:?}",
            data.minute_end
        );
    }

    #[test]
    fn link_type_with_brackets() {
        use crate::data::LinkType;
        let input = "[[https://example.com]]\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let para = section_children.first().expect("Expected paragraph");
        let para_children = &arena[*para].children;
        let link_node = para_children.first().expect("Expected link");
        let Syntax::Link(data) = &arena[*link_node].data else {
            panic!("Expected Link, got: {:?}", arena[*link_node].data);
        };
        assert!(
            matches!(data.link_type(), LinkType::File),
            "Expected LinkType::File for https link, got {:?}",
            data.link_type()
        );
    }
}

mod paragraph_edge_cases {
    use super::*;

    #[test]
    fn paragraph_with_dash_prefix() {
        let input = "para1\n----- text\npara2\n";
        let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 paragraph (----- text is not a horizontal rule), found {}",
            count
        );
    }

    #[test]
    fn paragraph_contains_list_item() {
        let input = "before\n- item\nafter\n";
        let list_count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            list_count, 1,
            "Expected 1 list when '- item' is mid-paragraph, found {}",
            list_count
        );
    }
}

mod granularity {
    use super::*;

    #[test]
    fn granularity_levels() {
        for (input, typ, granularity, desc) in &[
            (
                "* Headline\n\nSome paragraph\n",
                SyntaxT::Headline,
                ParseGranularity::Headline,
                "headline",
            ),
            (
                "* Headline\n\n#+BEGIN_CENTER\nCenter\n#+END_CENTER\n",
                SyntaxT::CenterBlock,
                ParseGranularity::GreaterElement,
                "greater element",
            ),
            (
                "* Headline\n\nParagraph text\n",
                SyntaxT::Paragraph,
                ParseGranularity::Element,
                "element",
            ),
            (
                "Paragraph with *bold* text\n",
                SyntaxT::Bold,
                ParseGranularity::Object,
                "object",
            ),
        ] {
            let count = get_type_count(input, *typ, *granularity);
            assert_eq!(
                count, 1,
                "Expected 1 node at {} granularity, found {}",
                desc, count
            );
        }
    }
}

/// Tests verifying coverage of the 21 Orgdown Level 1 (OD-1) syntax elements.
/// Reference: https://gitlab.com/publicvoit/orgdown/-/blob/master/doc/Orgdown1-Syntax-Examples.org
mod od1_compliance {
    use super::*;

    #[test]
    fn two_paragraphs_blank_line() {
        let input = "first paragraph\n\nsecond paragraph\n";
        let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert_eq!(
            count, 2,
            "Expected 2 paragraphs separated by blank line, found {}",
            count
        );
    }

    #[test]
    fn nested_markup_bold_contains_italic() {
        let input = "*bold /italic/ text*\n";
        let italic = get_type_count(input, SyntaxT::Italic, ParseGranularity::Object);
        assert_eq!(
            italic, 1,
            "Expected italic nested inside bold, found {}",
            italic
        );
    }

    #[test]
    fn multiple_markup_on_same_line() {
        let input = "*bold* and /italic/ and =code= and ~verbatim~\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        for (typ, desc) in &[
            (SyntaxT::Bold, "bold"),
            (SyntaxT::Italic, "italic"),
            (SyntaxT::Code, "code"),
            (SyntaxT::Verbatim, "verbatim"),
        ] {
            let count = count_type(&arena, root, *typ);
            assert_eq!(count, 1, "Expected 1 {}", desc);
        }
    }

    #[test]
    fn description_list_type_and_tag() {
        use crate::list::ListKind;
        let input = "- term :: description text\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section node");
        let section_ch = &arena[*section].children;
        let list = section_ch.first().expect("list node");
        let Syntax::PlainList(data) = &arena[*list].data else {
            panic!("Expected PlainList, got {:?}", arena[*list].data);
        };
        assert!(
            matches!(data.type_s, ListKind::Descriptive),
            "Expected Descriptive list type, got {:?}",
            data.type_s
        );
        let tag = &data.structure.items[0].tag;
        assert_eq!(
            *tag,
            Some("term"),
            "Item tag should be the term before '::' (got {:?})",
            tag
        );
    }

    #[test]
    fn all_five_block_types() {
        for (keyword, syntax_t) in &[
            ("EXAMPLE", SyntaxT::ExampleBlock),
            ("QUOTE", SyntaxT::QuoteBlock),
            ("VERSE", SyntaxT::VerseBlock),
            ("SRC", SyntaxT::SrcBlock),
            ("COMMENT", SyntaxT::CommentBlock),
        ] {
            let input = format!("#+BEGIN_{k}\nsome content\n#+END_{k}\n", k = keyword);
            let count = get_type_count(&input, *syntax_t, ParseGranularity::Element);
            assert_eq!(count, 1, "Expected 1 {} block, found {}", keyword, count);
        }
    }

    #[test]
    fn links_bare_bracketed_with_and_without_description() {
        for (input, desc, count) in &[
            (
                "Visit https://example.com today\n",
                "bare https URL",
                1_usize,
            ),
            ("[[https://example.com]]\n", "bracketed no description", 1),
            (
                "[[https://example.com][visit here]]\n",
                "bracketed with description",
                1,
            ),
            // Plain links use any registered Org link type, not just URLs.
            // From the Yantar92 corpus: `help:`, `file:` and `elisp:`.
            (
                "Reducing help:gcmh-cons-threshold now\n",
                "bare help: link",
                1,
            ),
            (
                "See file:~/.config/qutebrowser/urls here\n",
                "bare file: link",
                1,
            ),
            (
                "Run elisp:all-the-icons-install-fonts\n",
                "bare elisp: link",
                1,
            ),
        ] {
            let found = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
            assert_eq!(
                found, *count,
                "Expected {} for '{}', found {}",
                count, desc, found
            );
        }
    }

    /// In Emacs org-element a bracket link's description is stored in
    /// `org-element-contents`, so it must be parsed into child nodes.
    ///
    /// Corner cases:
    /// - plain text description → `PlainText` child
    /// - no description         → no children
    /// - description with bold  → `Bold` child (not just `PlainText`)
    mod link_description_as_children {
        use super::*;

        fn link_children(input: &str) -> Vec<SyntaxT> {
            let bump = Bump::new();
            let mut parser =
                Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
            let (arena, root) = parser.parse_buffer();
            // Walk to the Link node
            fn find_link(arena: &crate::data::NodeArena<'_, '_>, id: NodeId) -> Option<NodeId> {
                if SyntaxT::from(&arena[id].data) == SyntaxT::Link {
                    return Some(id);
                }
                for &child in &arena[id].children {
                    if let Some(l) = find_link(arena, child) {
                        return Some(l);
                    }
                }
                None
            }
            let link = find_link(&arena, root).expect("no Link found");
            arena[link]
                .children
                .iter()
                .map(|&id| SyntaxT::from(&arena[id].data))
                .collect()
        }

        #[test]
        fn link_description_children() {
            let children = link_children("[[https://example.com][OpenPGP]]\n");
            assert!(
                children.contains(&SyntaxT::PlainText),
                "expected PlainText child for plain description; got {:?}",
                children
            );
            let children = link_children("[[https://example.com]]\n");
            assert!(
                children.is_empty(),
                "link without description should have no children; got {:?}",
                children
            );
            let children = link_children("[[https://example.com][*bold*]]\n");
            assert!(
                children.contains(&SyntaxT::Bold),
                "expected Bold child for bold description; got {:?}",
                children
            );
        }

        #[test]
        fn link_description_no_nested_plain_link() {
            let input = "[[id:example][See https://example.com for details]]\n";
            let children = link_children(input);
            assert_eq!(
                children.len(),
                1,
                "expected single PlainText for entire description, got {:?}",
                children
            );
            assert_eq!(children[0], SyntaxT::PlainText);
        }
    }

    #[test]
    fn table_rows_including_hline() {
        let input = "| Name | Age |\n|------+-----|\n| Alice | 30 |\n";
        let row_count = get_type_count(input, SyntaxT::TableRow, ParseGranularity::Element);
        assert_eq!(
            row_count, 3,
            "Expected 3 rows (header + hline + data), found {}",
            row_count
        );
    }

    #[test]
    fn nested_list_inner_list_parsed() {
        let input = "- outer\n  - inner\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 2,
            "Expected outer and inner PlainList, found {}",
            count
        );
    }

    #[test]
    fn bold_in_various_containers() {
        for (input, desc) in &[
            ("- *bold* item\n", "list item"),
            ("| *bold* text |\n", "table cell"),
            ("#+BEGIN_QUOTE\n*bold*\n#+END_QUOTE\n", "quote block"),
            ("#+BEGIN_VERSE\n*bold*\n#+END_VERSE\n", "verse block"),
            ("#+BEGIN_CENTER\n*bold*\n#+END_CENTER\n", "center block"),
        ] {
            let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
            assert_eq!(count, 1, "Expected 1 bold inside {}, found {}", desc, count);
        }
    }

    #[test]
    fn headline_priority_extracted() {
        let input = "* [#A] important task\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let headline = root_children.first().expect("headline");
        let Syntax::Headline(data) = &arena[*headline].data else {
            panic!("Expected Headline, got {:?}", arena[*headline].data);
        };
        assert!(
            data.priority > 0,
            "Expected non-zero priority from [#A], got {}",
            data.priority
        );
        assert!(
            !data.title.contains("[#A]"),
            "Priority cookie should not appear in title, got {:?}",
            data.title
        );
    }

    #[test]
    fn block_end_must_match_block_type() {
        // #+END_SRC on its own line must not close a #+BEGIN_QUOTE block.
        // With the bug, the quote closes early and "more" / "#+END_QUOTE"
        // become stray section-level paragraphs.
        let input = "#+BEGIN_QUOTE\n#+END_SRC\nmore\n#+END_QUOTE\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section");
        let para_count = arena[*section]
            .children
            .iter()
            .filter(|&&n| matches!(arena[n].data, Syntax::Paragraph))
            .count();
        assert_eq!(
            para_count, 0,
            "#+END_SRC should not terminate #+BEGIN_QUOTE; {} stray paragraph(s) at section level",
            para_count
        );
    }

    #[test]
    fn planning_newline_consumed() {
        // planning_parser sets location.end to the position OF '\n', not past it.
        // That '\n' is then re-parsed as its own element, producing a phantom
        // empty paragraph between the planning line and the actual content.
        let input = "* Headline\nDEADLINE: <2023-12-31>\ncontent\n";
        let para_count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert_eq!(
            para_count, 1,
            "Expected 1 paragraph after planning line, found {} (phantom newline paragraph?)",
            para_count
        );
    }

    #[test]
    fn bold_in_headline_title() {
        let input = "* *bold* heading\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let hl = *arena[root].children.first().expect("expected headline");
        let crate::data::Syntax::Headline(ref data) = arena[hl].data else {
            panic!("expected Headline");
        };
        let has_bold = data
            .title_objects
            .iter()
            .any(|&id| SyntaxT::from(&arena[id].data) == SyntaxT::Bold);
        assert!(
            has_bold,
            "expected Bold in title_objects; got {:?}",
            data.title_objects
                .iter()
                .map(|&id| SyntaxT::from(&arena[id].data))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn double_blank_line_two_paragraphs() {
        let input = "para1\n\n\npara2\n";
        let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert_eq!(
            count, 2,
            "Two blank lines should still give 2 paragraphs, found {}",
            count
        );
    }

    #[test]
    fn bold_in_footnote_reference_inline_definition() {
        let input = "text [fn::*bold* note] end\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert_eq!(
            count, 1,
            "Expected 1 bold inside footnote inline definition, found {}",
            count
        );
    }

    #[test]
    fn timestamp_with_dayname_and_time() {
        let input = "<2023-12-31 Sun 10:30>\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section");
        let section_ch = &arena[*section].children;
        let para = section_ch.first().expect("para");
        let para_ch = &arena[*para].children;
        let ts = para_ch
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::Timestamp(_)))
            .expect("timestamp");
        let Syntax::Timestamp(data) = &arena[*ts].data else {
            panic!("Expected Timestamp");
        };
        assert_eq!(
            data.hour_start,
            Some(10),
            "Expected hour 10 from '<2023-12-31 Sun 10:30>', got {:?}",
            data.hour_start
        );
    }

    #[test]
    fn table_does_not_swallow_following_paragraph() {
        // table_parser initialises `end = start` and never updates it inside
        // the row loop.  After the loop it does `if end == start { end = limit }`,
        // so the table always claims location.end == limit.  Any element that
        // follows the table within the same section is never parsed.
        let input = "| a | b |\n| c | d |\nparagraph\n";
        let para_count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert_eq!(
            para_count, 1,
            "paragraph after table should be a sibling, found {} paragraphs",
            para_count
        );
    }

    #[test]
    fn planning_deadline_dropped_when_closed_follows_on_same_line() {
        // parse_timestamp_from_str slices from the keyword to end of line.
        // When DEADLINE: <date> CLOSED: [date] appear on the same line,
        // the DEADLINE slice ends with ']' (the CLOSED bracket), so the
        // `starts_with('<') && ends_with('>')` guard fails and deadline is
        // silently set to None even though it was explicitly present.
        let input = "* Head\nDEADLINE: <2023-12-31> CLOSED: [2024-01-01]\ncontent\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let root_ch = &arena[root].children;
        let headline = root_ch.first().expect("headline");
        let hl_ch = &arena[*headline].children;
        let section = hl_ch
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::Section))
            .expect("section");
        let sec_ch = &arena[*section].children;
        let planning = sec_ch
            .iter()
            .find(|&&n| matches!(arena[n].data, Syntax::Planning(_)))
            .expect("planning node");
        let Syntax::Planning(p) = &arena[*planning].data else {
            panic!("expected Planning node");
        };
        assert!(
            p.deadline.is_some(),
            "DEADLINE should be parsed even when CLOSED follows on the same line"
        );
    }
}

/// Subscript (`text_{sub}` / `text_word`) and superscript (`text^{sup}`)
/// objects are defined in `SyntaxT` but never emitted by the parser.
///
/// Corpus discrepancy class:
///   `root/headline/section/paragraph/subscript` (3)
///   `root/headline/section/quote_block/paragraph/subscript` (11)
///
/// Corner cases:
/// - bare word subscript: `H_2` → subscript `2`
/// - braced subscript: `H_{2}O` → subscript `2`
/// - bare superscript: `E=mc^2` → superscript `2`
/// - braced superscript: `x^{n+1}` → superscript `n+1`
mod subscript_superscript {
    use super::*;
    use crate::data::ScriptKind;

    fn find_script(arena: &crate::data::NodeArena, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        if matches!(arena[id].data, Syntax::Script(_)) {
            out.push(id);
        }
        for &child in &arena[id].children {
            out.extend(find_script(arena, child));
        }
        out
    }

    #[test]
    fn script_nodes_detected() {
        for (input, expected_kind, desc) in &[
            ("H_2\n", Some(ScriptKind::Sub), "bare subscript H_2"),
            ("E=mc^2\n", Some(ScriptKind::Sup), "bare superscript mc^2"),
            ("x^{n+1}\n", None, "braced superscript x^{n+1}"),
            // Emacs [[:alnum:]] matches Unicode (Chinese, Cyrillic, etc.);
            // Rust uses is_ascii_alphanumeric() which misses these.
            ("?^字母\n", Some(ScriptKind::Sup), "superscript with CJK chars"),
            ("x^αβγ\n", Some(ScriptKind::Sup), "superscript with Greek letters"),
        ] {
            let bump = Bump::new();
            let mut parser =
                Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
            let (arena, root) = parser.parse_buffer();
            let scripts = find_script(&arena, root);
            assert!(
                !scripts.is_empty(),
                "expected a Script node for {}; got none",
                desc
            );
            if let Some(kind) = expected_kind {
                let actual = if let Syntax::Script(f) = arena[scripts[0]].data {
                    f.kind()
                } else {
                    unreachable!()
                };
                assert_eq!(
                    actual, *kind,
                    "expected {:?} for {}, got {:?}",
                    kind, desc, actual
                );
            }
        }
    }

    #[test]
    fn subscript_has_plain_text_child() {
        // bare: "x_foo test\n" — Emacs: subscript at [1,6) with plain-text child "foo" at [2,5).
        // braced: "H_{2}O\n" — braced subscript {2} must contain a PlainText child.
        for (input, desc) in &[("x_foo test\n", "bare"), ("H_{2}O\n", "braced")] {
            let bump = Bump::new();
            let mut parser =
                Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
            let (arena, root) = parser.parse_buffer();
            let scripts = find_script(&arena, root);
            assert!(
                !scripts.is_empty(),
                "no Script node found for {} subscript",
                desc
            );
            let script_id = scripts[0];
            let children = &arena[script_id].children;
            assert_eq!(
                children.len(),
                1,
                "Script must have exactly 1 PlainText child for {} subscript, got {}",
                desc,
                children.len()
            );
            assert!(
                matches!(arena[children[0]].data, Syntax::PlainText(_)),
                "Script child must be PlainText for {} subscript",
                desc
            );
        }
    }
}

mod entity {
    use super::*;

    #[test]
    fn le_is_entity_not_latex_fragment() {
        // \le is an org entity (≤), not a LaTeX fragment
        let count = get_type_count(r"\le100Mb", SyntaxT::Entity, ParseGranularity::Object);
        assert_eq!(count, 1, "\\le should parse as Entity, not LatexFragment");
        let count_lf = get_type_count(
            r"\le100Mb",
            SyntaxT::LatexFragment,
            ParseGranularity::Object,
        );
        assert_eq!(count_lf, 0, "\\le should not produce a LatexFragment");
    }

    #[test]
    fn s_is_entity_not_latex_fragment() {
        // \S is an org entity (§), not a LaTeX fragment.
        // Emacs's entity table includes single-letter entries like "S" (section sign),
        // but org-rs's EntityData::new() is missing them, so \S falls through to
        // try_parse_latex_fragment and gets parsed as LatexFragment instead.
        let count = get_type_count(r"\S100Mb", SyntaxT::Entity, ParseGranularity::Object);
        assert_eq!(count, 1, "\\S should parse as Entity, not LatexFragment");
        let count_lf = get_type_count(
            r"\S100Mb",
            SyntaxT::LatexFragment,
            ParseGranularity::Object,
        );
        assert_eq!(count_lf, 0, "\\S should not produce a LatexFragment");
    }
}

mod post_blank {

    use super::*;

    fn plain_text_starts(input: &str) -> Vec<usize> {
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let mut starts = Vec::new();
        collect_plain_text_starts(&arena, root, &mut starts);
        starts
    }

    fn collect_plain_text_starts(arena: &crate::data::NodeArena, id: NodeId, out: &mut Vec<usize>) {
        if matches!(arena[id].data, Syntax::PlainText(_)) {
            out.push(arena[id].location.start);
        }
        for &child in &arena[id].children {
            collect_plain_text_starts(arena, child, out);
        }
    }

    #[test]
    fn plain_text_after_markup_starts_after_space() {
        // "text *bold* word\n": plain_text at 12 ('w'), not 11 (' ')
        //  0    5    10 11 12
        let starts = plain_text_starts("text *bold* word\n");
        assert!(
            starts.contains(&12),
            "expected plain_text at 12 ('w'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&11),
            "plain_text must not start at 11 (the post-blank space), got starts: {:?}",
            starts
        );

        // "see ~foo~ bar\n": plain_text at 10 ('b'), not 9 (' ')
        //  0   4   8 9 10
        let starts = plain_text_starts("see ~foo~ bar\n");
        assert!(
            starts.contains(&10),
            "expected plain_text at 10 ('b'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&9),
            "plain_text must not start at 9 (post-blank space), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn bold_followed_by_double_quote() {
        // Emacs includes `"` as a valid post-char for emphasis markers.
        // Corpus: `"...-*"` with `"` immediately after closing `*`.
        let input = "\"*bold*\" text\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert_eq!(
            count, 1,
            "Expected 1 bold with double-quote post-char, got {}",
            count
        );
    }

    #[test]
    fn no_space_after_bold_unaffected() {
        // "a *bold*.\n" — post-char is '.', not a space; no post_blank to skip
        //  0 2      89
        //           ^-- '.': plain_text starts immediately after bold
        let starts = plain_text_starts("a *bold*.\n");
        assert!(
            starts.contains(&8),
            "expected plain_text at 8 ('.'), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn failed_markup_does_not_split_plain_text() {
        // "text + more\n": bare '+' at 5 with no closer must not start a new PlainText.
        //  0    5
        let starts = plain_text_starts("text + more\n");
        assert!(
            !starts.contains(&5),
            "plain_text must not start at 5 (the bare '+'); got starts: {:?}",
            starts
        );

        // "foo\n/bar\n": bare '/' at 4 with no closer must not start a new PlainText.
        //  0   3 4
        let starts = plain_text_starts("foo\n/bar\n");
        assert!(
            !starts.contains(&4),
            "plain_text must not start at 4 (the bare '/'); got starts: {:?}",
            starts
        );
    }

    #[test]
    fn plain_text_after_bracket_link_starts_after_space() {
        // "[[help:consult-outline][consult-outline]] / [[help:consult-imenu][consult-imenu]].\n"
        //  0                                      39^40^41^42
        //                                         ']' ']' ' ' '/': byte layout
        //
        // Bytes 0-40: the link [[help:consult-outline][consult-outline]] (41 bytes).
        // Byte 41: ' ' (space) — Emacs absorbs this into the link's :end (post-blank).
        // Byte 42: '/' — Emacs plain_text begin.
        //
        // Emacs org-element absorbs trailing whitespace into a link's :end (:post-blank),
        // so the plain_text "/ " starts at byte 42 (the '/').
        // Rust does not absorb post-blank in try_parse_link, so it ends the link at
        // byte 41 (exclusive).  scan_plain_text_end on " / [[..." stops at index 1
        // because '/' at index 1 is preceded by ' ' (a pre-char), splitting off a
        // lone 1-char plain_text at byte 41 that Emacs does not produce.
        let input =
            "[[help:consult-outline][consult-outline]] / [[help:consult-imenu][consult-imenu]].\n";
        let starts = plain_text_starts(input);
        assert!(
            starts.contains(&42),
            "expected plain_text at 42 ('/'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&41),
            "plain_text must not start at 41 (post-blank space after link), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn bracket_not_link_does_not_split_plain_text() {
        // "foo [bar] baz\n" — `[bar]` is not a link.
        // `[` must not split the PlainText; byte 5 (`b`) must not be a PlainText start.
        let input = "foo [bar] baz\n";
        let starts = plain_text_starts(input);
        assert!(
            !starts.contains(&5),
            "PlainText must not start at byte 5 (after `[`); `[` should be included in preceding PlainText. starts: {:?}",
            starts
        );
    }

    #[test]
    fn plain_link_absorbs_trailing_spaces() {
        // "https://example.com  \nrest\n"
        // Emacs absorbs the two trailing spaces into the link as post-blank.
        // The PlainText for '\n' must start at byte 21 (the `\n`), not at 19 (the first space).
        let input = "https://example.com  \nrest\n";
        let starts = plain_text_starts(input);
        assert!(
            !starts.contains(&19),
            "PlainText must not start at byte 19 (first trailing space); spaces must be absorbed by plain link. starts: {:?}",
            starts
        );
    }

    #[test]
    fn plain_text_after_timestamp_starts_after_space() {
        // "[2021-05-05 Wed] Discussion\n"
        //  0                15 16 17
        //                    ]  ' ' 'D'
        // Emacs absorbs the trailing space into the timestamp as post-blank,
        // so PlainText starts at byte 17 ('D'), not byte 16 (the space).
        let starts = plain_text_starts("[2021-05-05 Wed] Discussion\n");
        assert!(
            starts.contains(&17),
            "expected plain_text at 17 ('D'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&16),
            "plain_text must not start at 16 (post-blank space after timestamp), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn plain_text_after_timestamp_with_extra_spaces() {
        // "[2021-05-05 Wed]   Discussion\n"
        // Emacs absorbs both spaces into the timestamp as post-blank.
        let starts = plain_text_starts("[2021-05-05 Wed]   Discussion\n");
        assert!(
            starts.contains(&19),
            "expected plain_text at 19 ('D'), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn plain_text_after_timestamp_no_space() {
        // "[2021-05-05 Wed]Discussion\n"
        // No post-char after `]` so this is NOT a valid timestamp.
        // The entire string remains as a single PlainText run.
        let starts = plain_text_starts("[2021-05-05 Wed]Discussion\n");
        assert_eq!(
            starts,
            vec![0],
            "no timestamp split expected when `]` is directly followed by a non-post-char, got starts: {:?}",
            starts
        );
    }

    #[test]
    fn paragraph_boundary_at_blank_line() {
        // "text.\n\nMore\n"
        //  0    5 6 7
        //            ^ 'M'
        // Emacs absorbs the blank line as post-blank of the first paragraph:
        //   Paragraph (0,7)  — location spans "text.\n\n", post_blank=1
        //   Paragraph (7,12) — spans "More\n"
        let bump = Bump::new();
        let mut parser = Parser::new(
            "text.\n\nMore\n",
            ParseGranularity::Element,
            DefaultEnvironment,
            &bump,
        );
        let (arena, root) = parser.parse_buffer();
        let section = arena[root].children.first().expect("section");
        let paras: Vec<_> = arena[*section]
            .children
            .iter()
            .map(|&id| (&arena[id], arena[id].post_blank))
            .collect();
        assert_eq!(paras.len(), 2, "expected 2 paragraphs");
        assert_eq!(paras[0].0.location.start, 0);
        assert_eq!(
            paras[0].0.location.end, 7,
            "first paragraph location should include the blank line"
        );
        assert_eq!(paras[0].1, 1, "first paragraph should have post_blank=1");
        assert_eq!(paras[1].0.location.start, 7);
        assert_eq!(paras[1].0.location.end, 12);
    }

    #[test]
    fn plain_link_does_not_absorb_trailing_punctuation() {
        // "https://example.com. more\n"
        //  0           18 19 20 21
        // The '.' at 19 is NOT part of the plain link in Emacs —
        // it is trailing punctuation that starts a new PlainText run.
        // Rust currently absorbs '.' into the link and starts PlainText at 21.
        let starts = plain_text_starts("https://example.com. more\n");
        assert!(
            starts.contains(&19),
            "expected PlainText at byte 19 (the '.' after the URL), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&21),
            "PlainText must not start at byte 21 (the space should be absorbed as post-blank), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn angle_link_parsed_as_link_object() {
        // "<http://example.com> text\n"
        //  0                18 19 20 21
        //         (url ends) > ' ' 't'
        // Emacs parses this as an Angle link, absorbing trailing space as post-blank.
        let starts = plain_text_starts("<http://example.com> text\n");
        assert!(
            starts.contains(&21),
            "expected PlainText at byte 21 ('text'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&0),
            "expected no PlainText at 0 — angle link should be parsed first, got starts: {:?}",
            starts
        );
    }

    #[test]
    fn latex_fragment_parsed_as_object() {
        // "\texttt{C-c ,} rest\n"
        //  0                14 15
        //                   } ' ' 'r'
        // Emacs parses \texttt{...} as a LatexFragment object.
        // The string is 14 bytes (\ + 6-letter "texttt" + {C-c ,}),
        // + 1 trailing space absorbed as post-blank = total consumed 15.
        let starts = plain_text_starts(r"\texttt{C-c ,} rest\n");
        assert!(
            starts.contains(&15),
            "expected PlainText at byte 15 ('rest'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&0),
            "expected no PlainText at 0 — LaTeX fragment should be parsed first, got starts: {:?}",
            starts
        );
    }

    #[test]
    fn latex_fragment_bare_command() {
        // "\Program rest\n"
        //  0         8  9
        //           ' ' 'r'
        // Emacs parses \Program (no braces) as a LatexFragment object.
        let starts = plain_text_starts(r"\Program rest\n");
        assert!(
            starts.contains(&9),
            "expected PlainText at byte 9 ('rest'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&0),
            "expected no PlainText at 0 — LaTeX fragment should be parsed first, got starts: {:?}",
            starts
        );
    }

    #[test]
    fn latex_fragment_non_entity_not_stolen() {
        // "\notanentity rest\n"
        //  0            11 12
        // Bare LaTeX fragment should handle \notanentity (not a known entity),
        // consuming it + trailing space → PlainText at 13.
        let starts = plain_text_starts(r"\notanentity rest\n");
        assert_eq!(
            starts,
            vec![13],
            "expected PlainText at byte 13 ('rest'), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn citation_parsed_as_object() {
        // "[cite/t:@all] rest\n"
        //  0            12 13 14
        //                ] ' ' 'r'
        // Emacs parses as Citation object, absorbing trailing space as post-blank.
        let starts = plain_text_starts("[cite/t:@all] rest\n");
        assert!(
            starts.contains(&14),
            "expected PlainText at byte 14 ('rest'), got starts: {:?}",
            starts
        );
        assert!(
            !starts.contains(&0),
            "expected no PlainText at 0 — citation should be parsed first, got starts: {:?}",
            starts
        );
    }

    #[test]
    fn statistics_cookie_not_plain_text() {
        // "text [/] rest\n"
        //  0    5   8 9
        // Emacs parses [/] as StatisticsCookie object.
        // Rust currently lacks a StatisticsCookie parser.
        let count = get_type_count(
            "text [/] rest\n",
            SyntaxT::StatisticsCookie,
            ParseGranularity::Object,
        );
        assert_eq!(
            count, 1,
            "expected 1 StatisticsCookie for '[/]', got {} — StatisticsCookie parser not implemented",
            count
        );
    }

    #[test]
    fn underline_not_parsed_after_alphanumeric() {
        // From `karl_voit_config.org`: in `(#+BEGIN_... and #+END_...)` the
        // `_` after `BEGIN` must NOT open an Underline, because the preceding
        // char (`N`) is not a valid emphasis pre-character. Emacs emits no
        // underline here; Rust did because it skipped the pre-condition check.
        let count = get_type_count(
            "within a block (#+BEGIN_... and #+END_...).\n",
            SyntaxT::Underline,
            ParseGranularity::Object,
        );
        assert_eq!(
            count, 0,
            "expected 0 Underline for `#+BEGIN_... #+END_...`, got {} — emphasis pre-condition not checked",
            count
        );
    }

    #[test]
    fn citation_reference_inside_citation() {
        // Emacs parses `[cite/t:@all]` as a Citation that contains a
        // CitationReference (`@all`). Rust builds the Citation node but
        // never parses its reference child — the Rust data model has no
        // CitationReference variant, so the citation comes out childless.
        let bump = Bump::new();
        let mut parser = Parser::new(
            "Hi [cite/t:@all], talk.\n",
            ParseGranularity::Object,
            DefaultEnvironment,
            &bump,
        );
        let (arena, root) = parser.parse_buffer();

        fn find_citation(arena: &NodeArena, id: NodeId) -> Option<NodeId> {
            if matches!(arena[id].data, Syntax::Citation(_)) {
                return Some(id);
            }
            arena[id]
                .children
                .iter()
                .find_map(|&c| find_citation(arena, c))
        }
        let citation = find_citation(&arena, root).expect("expected a Citation node");
        assert!(
            !arena[citation].children.is_empty(),
            "expected the Citation to contain a CitationReference child, got none",
        );
    }

    #[test]
    fn latex_fragment_command_stops_before_digits() {
        // `\Office16` — Emacs ends the LaTeX command name at the last
        // letter (`\Office`) and leaves `16` as plain-text. Rust's command
        // scan uses is_ascii_alphanumeric and swallows the trailing digits.
        //  \Office16\OUTLOOK rest
        //  0      7  9
        let starts = plain_text_starts(r"\Office16\OUTLOOK rest\n");
        assert!(
            starts.contains(&7),
            "expected PlainText '16' at byte 7 (LaTeX command must stop before digits), got starts: {:?}",
            starts
        );
    }

    #[test]
    fn statistics_cookie_in_link_description() {
        // `[[id:...][[/] focus proj]]` — the link description contains a
        // statistics cookie `[/]`. Emacs parses it as a StatisticsCookie
        // object inside the link; Rust keeps the description as plain-text.
        let count = get_type_count(
            "[[id:2021-07-27-focus-proj][[/] focus proj]]\n",
            SyntaxT::StatisticsCookie,
            ParseGranularity::Object,
        );
        assert_eq!(
            count, 1,
            "expected 1 StatisticsCookie inside the link description, got {} — link descriptions not scanned for objects",
            count
        );
    }

/// Find the first PlainList child of an Item node.
fn item_a_plainlist_child<'a>(arena: &'a NodeArena, item_id: NodeId) -> Option<NodeId> {
    arena[item_id]
        .children
        .iter()
        .copied()
        .find(|&id| matches!(arena[id].data, Syntax::PlainList(_)))
}

/// Items with indentation that goes UP then DOWN must resolve to the
/// correct parent.  For example with `- a`, `  - b` (indent 2), ` - c`
/// (indent 1), item `c` is a sibling of `b` — both children of `a` —
/// NOT a child of `b`.  Rust's current indent-grouping approach
/// incorrectly nests `c` inside `b`.
///
/// Correct structure (2 PlainLists):
///   (plain-list
///     (item "a"
///       (plain-list
///         (item "b")
///         (item "c"))))
#[test]
fn list_indent_goes_up_then_down() {
    let input = "- a\n  - b\n - c\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let pl_count = count_type(&arena, root, SyntaxT::PlainList);
    // Items b (indent 2) and c (indent 1) share parent a → one sub-list
    assert_eq!(
        pl_count, 2,
        "Expected 2 PlainLists (outer + 1 sub-list), got {} — indent-up-then-down items \
         should be siblings, not nested",
        pl_count
    );
    // Check that both items are direct children of the sub-list, not nested
    // Walk: section > plain_list > item > plain_list > [items]
    assert!(arena[root].children.len() >= 1, "expected section children");
    let sec = arena[root].children[0];
    let pl = arena[sec].children[0];
    let item_a = arena[pl].children[0];
    let inner_pl = item_a_plainlist_child(&arena, item_a)
        .expect("item 'a' must contain a PlainList child");
    assert_eq!(
        arena[inner_pl].children.len(),
        2,
        "inner PlainList should contain exactly 2 items (b and c), not nested items; got {}",
        arena[inner_pl].children.len()
    );
}

    /// Same issue with `*` bullets and tab-derived indentation.  Items at
    /// indent 11 followed by indent 10 should be siblings, not nested.
    ///
    /// Correct structure (3 PlainLists):
    ///   (plain-list
    ///     (item "a"
    ///       (plain-list           ← children of a: b (indent 11) and c (indent 10)
    ///         (item "b")
    ///         (item "c"
    ///           (plain-list       ← child of c: d (indent 11)
    ///             (item "d"))))))
    #[test]
    fn star_list_indent_goes_up_then_down() {
        let input = "\t* a\n   \t* b\n\t  * c\n   \t* d\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let pl_count = count_type(&arena, root, SyntaxT::PlainList);
        assert_eq!(
            pl_count, 3,
            "Expected 3 PlainLists (outer + sub for b,c + sub for d), got {}",
            pl_count
        );
        // Verify nesting: b and c are siblings, d is under c
        let sec = arena[root].children[0];
        let outer_pl = arena[sec].children[0];
        let item_a = arena[outer_pl].children[0];
        // item_a children: Paragraph("a") + PlainList
        let inner_pl_idx = item_a_plainlist_child(&arena, item_a)
            .expect("item 'a' must contain a PlainList child");
        let inner_pl = inner_pl_idx;
        // inner_pl should contain b and c as direct children
        assert_eq!(
            arena[inner_pl].children.len(),
            2,
            "inner PlainList should contain exactly 2 items (b and c), not 1 with c nested inside; got {}",
            arena[inner_pl].children.len()
        );
        // item d should be nested under item c
        let item_c = arena[inner_pl].children[1];
        let c_has_sub = arena[item_c].children.iter().any(|&id| {
            matches!(arena[id].data, Syntax::PlainList(_))
        });
        assert!(
            c_has_sub,
            "item 'c' must contain a sub-PlainList for item 'd'"
        );
    }

    #[test]
    fn name_affiliated_keyword_not_plain_text() {
        let starts = plain_text_starts(
            "text\n\n#+name: url\nhttps://example.com\n#+begin_src bash\necho 1\n#+end_src\n",
        );
        assert!(
            !starts.contains(&6),
            "expected no PlainText at byte 6 (#+name: line), got starts: {:?}",
            starts
        );
    }
}

/// Corpus discrepancy class:
///   `root/headline/section/plain_list` — missing in Rust when a lower-indented
///   bullet terminates the outer list.
///
/// When a list item has a smaller string-width indent than the list's first
/// item, Emacs' `org-list-struct` terminates the current list.  Rust currently
/// keeps scanning, folding the lower-indent item into the same structure.
mod list_lower_indent_termination {
    use super::*;

    /// `   \t *` has string-width indent 12; `       *` has indent 7.
    /// Because 7 < 12, Emacs ends the first list and starts a second one.
    /// Corpus origin: emacs_china_elisp.org @4912/@4945 pattern.
    #[test]
    fn lower_indent_bullet_starts_new_list() {
        let input = "   \t * item1\n       * item2\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let pl = count_type(&arena, root, SyntaxT::PlainList);
        assert_eq!(
            pl, 2,
            "lower-indent bullet must start a new PlainList; got {pl} (expected 2)"
        );
    }
}

/// Corpus discrepancy class:
///   `root/headline/section/fixed_width` vs `root/headline/section/keyword` —
///   `  #+RESULTS:` with leading whitespace is not collected as an affiliated
///   keyword, so the `: output` line is parsed as a standalone FixedWidth
///   instead of one whose `:begin` includes the `#+RESULTS:` line.
///
/// Corpus origin: emacs_china_elisp.org @323688.
mod affiliated_keyword_with_leading_whitespace {
    use super::*;

    /// `  #+RESULTS:` has leading whitespace before `#+`.
    /// Emacs: one FixedWidth element with `:begin` at the `#+RESULTS:` line.
    /// Rust:  a Keyword element + a separate FixedWidth element.
    #[test]
    fn indented_results_keyword_affiliated_to_fixed_width() {
        let input = "  #+RESULTS:\n  : output\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let kw = count_type(&arena, root, SyntaxT::Keyword);
        let fw = count_type(&arena, root, SyntaxT::FixedWidth);
        assert_eq!(
            kw, 0,
            "#+RESULTS: with leading spaces must be an affiliated keyword, not a Keyword element; got {kw} keywords"
        );
        assert_eq!(
            fw, 1,
            "expected exactly 1 FixedWidth element (with #+RESULTS: as affiliated keyword); got {fw}"
        );
    }
}

/// Corpus discrepancy class:
///   `root/headline/section/drawer/paragraph` — extra paragraph at the drawer
///   header line when `#+RESULTS:` is an affiliated keyword.
///
/// When `drawer_content_bounds` uses `span.start` (the affiliated keyword
/// position) instead of `content_start` (the `:DRAWERNAM:` position), the
/// content region begins at the drawer header line.  The parser then tries
/// to parse `:RESULTS:` as a nested element, fails, and creates a fallback
/// paragraph there.
///
/// Corpus origin: emacs_china_elisp.org @125432.
mod drawer_affiliated_keyword_content {
    use super::*;

    /// `#+RESULTS:\n:RESULTS:\ncontent\n:END:\n`
    /// Emacs: one Drawer with one Paragraph child (`content`).
    /// Rust (buggy): Drawer has two Paragraph children (`:RESULTS:` line + `content`).
    #[test]
    fn drawer_with_results_affiliated_has_one_paragraph() {
        let input = "#+RESULTS:\n:RESULTS:\ncontent\n:END:\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        let drawers: Vec<NodeId> = {
            let mut out = Vec::new();
            fn collect(arena: &NodeArena, id: NodeId, out: &mut Vec<NodeId>) {
                if matches!(arena[id].data, Syntax::Drawer(_)) {
                    out.push(id);
                }
                for &c in &arena[id].children {
                    collect(arena, c, out);
                }
            }
            collect(&arena, root, &mut out);
            out
        };

        assert_eq!(drawers.len(), 1, "expected exactly 1 Drawer; got {}", drawers.len());
        let drawer_id = drawers[0];
        let para_count = count_type(&arena, drawer_id, SyntaxT::Paragraph);
        assert_eq!(
            para_count, 1,
            "Drawer must have exactly 1 Paragraph child (the content, not the header line); got {para_count}"
        );
    }
}
