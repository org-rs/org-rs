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
    fn nested_list() {
        let input = "- item 1\n  - subitem 1\n  - subitem 2\n- item 2\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let count = count_type(&arena, root, SyntaxT::PlainList);
        let item_count = count_type(&arena, root, SyntaxT::Item);
        assert!(
            count >= 2,
            "Expected at least 2 PlainLists (outer + inner), found {}",
            count
        );
        assert!(
            item_count >= 3,
            "Expected at least 3 items, found {}",
            item_count
        );
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
            t: SyntaxT,
        ) -> Option<NodeId> {
            if SyntaxT::from(&arena[id].data) == t {
                return Some(id);
            }
            arena[id]
                .children
                .iter()
                .find_map(|&c| find_first(arena, c, t))
        }

        let row = find_first(&arena, root, SyntaxT::TableRow).expect("TableRow not found");
        for &child in &arena[row].children {
            assert_eq!(
                SyntaxT::from(&arena[child].data),
                SyntaxT::TableCell,
                "direct child of TableRow must be TableCell, got {:?}",
                SyntaxT::from(&arena[child].data)
            );
        }
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
}
