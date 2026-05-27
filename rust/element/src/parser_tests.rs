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
    let mut parser = Parser::new(input, granularity, DefaultEnvironment);
    let (arena, root) = parser.parse_buffer();
    count_type(&arena, root, typ)
}

mod plain_list {
    use super::*;

    #[test]
    fn unordered_list() {
        let input = "- item 1\n- item 2\n- item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn ordered_list() {
        let input = "1. item 1\n2. item 2\n3. item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn descriptive_list() {
        let input = "- term 1 :: description 1\n- term 2 :: description 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn list_with_checkbox() {
        let input = "- [X] checked item\n- [ ] unchecked item\n- [-] partially checked\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn nested_list() {
        let input = "- item 1\n  - subitem 1\n  - subitem 2\n- item 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        let item_count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(
            item_count >= 1 || count >= 1,
            "Expected list or items, found list: {}, items: {}",
            count,
            item_count
        );
    }

    #[test]
    fn list_two_items() {
        let input = "- item 1\n- item 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn list_empty() {
        let input = "- \n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert!(count > 0, "Plain list count: {}", count);
    }

    #[test]
    fn numbered_list_over_ten() {
        let input = "10. item\n11. another\n12. yet another\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 plain list with items >=10, found {}",
            count
        );
        let item_count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(
            item_count >= 3,
            "Expected at least 3 items for list items >=10, found {}",
            item_count
        );
    }
}

mod item {
    use super::*;

    #[test]
    fn simple_item() {
        let input = "- some text\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn item_with_bullet() {
        let input = "+ some text\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count > 0, "Item count: {}", count);
    }

    #[test]
    fn item_with_tag() {
        let input = "- tag :: content\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count > 0, "Item count: {}", count);
    }

    #[test]
    fn item_with_checkbox() {
        let input = "- [X] checked item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn item_with_unchecked() {
        let input = "- [ ] unchecked item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn item_ordered_start() {
        let input = "1. first item\n2. second item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 2, "Expected at least 2 items, found {}", count);
    }

    #[test]
    fn item_with_counter_set() {
        let input = "1. [@3] item\n2. next\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let list_node = section_children.first().expect("Expected plain list");
        let list_children = &arena[*list_node].children;
        let first_item = list_children.first().expect("Expected first item");

        if let Syntax::Item(data) = &arena[*first_item].data {
            assert_eq!(
                data.counter, 3,
                "Item counter should be 3 from [@3], got {}",
                data.counter
            );
        } else {
            panic!("Expected Item, got: {:?}", arena[*first_item].data);
        }
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
        /// not be swallowed into the item's content.
        #[test]
        fn sub_headline_after_blank_is_not_item_child() {
            let input = "* Top Level\n- list item\n\n** Sub-headline\n";
            let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();

            let top_hl = arena[root].children[0];
            let section = arena[top_hl].children
                .iter()
                .find(|&&n| matches!(arena[n].data, Syntax::Section))
                .copied()
                .expect("expected Section child of headline");
            let list = arena[section].children
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
        }

        /// The sub-headline must still appear as a sibling of the section
        /// (direct child of the parent headline), not lost entirely.
        #[test]
        fn sub_headline_is_sibling_of_section() {
            let input = "* Top Level\n- list item\n\n** Sub-headline\n";
            let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();

            let top_hl = arena[root].children[0];
            let sub_hl_count = arena[top_hl].children
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
            let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();

            let top_hl = arena[root].children[0];
            let section = arena[top_hl].children
                .iter()
                .find(|&&n| matches!(arena[n].data, Syntax::Section))
                .copied()
                .expect("expected Section");
            let list = arena[section].children
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
            let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();

            // Find the PlainList (may be inside a section or directly under root)
            fn find_list(arena: &crate::data::NodeArena, id: NodeId) -> Option<NodeId> {
                if matches!(arena[id].data, Syntax::PlainList(_)) { return Some(id); }
                for &c in &arena[id].children {
                    if let Some(l) = find_list(arena, c) { return Some(l); }
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
            let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();
            // Walk: root → section → plain_list → item → paragraph
            let section  = arena[root].children[0];
            let list     = arena[section].children[0];
            let item     = arena[list].children[0];
            let para     = arena[item].children[0];
            assert_eq!(
                SyntaxT::from(&arena[para].data),
                SyntaxT::Paragraph,
                "expected Paragraph as first child of item"
            );
            arena[para].location.start
        }

        /// Basic case: `- tag :: content` — paragraph starts at `content`.
        #[test]
        fn basic_desc_para_start() {
            //          0123456789012345
            let input = "- tag :: content\n";
            //                   ^ offset 9
            assert_eq!(para_start(input), 9, "paragraph should start after ' :: '");
        }

        /// Greedy: Emacs matches the LAST ` :: ` on the line as the separator.
        /// So `- A :: B :: C` has tag `A :: B` and content `C`.
        #[test]
        fn greedy_last_separator() {
            //          0123456789012345678901
            let input = "- A :: B :: C\n";
            //                       ^ offset 12
            assert_eq!(para_start(input), 12, "greedy: last ' :: ' is the separator");
        }

        /// Tab after `::`: `- tag ::\tcontent` — paragraph starts at `content`.
        #[test]
        fn tab_after_colons() {
            let input = "- tag ::\tcontent\n";
            //           0123456789
            //                    ^ offset 9
            assert_eq!(para_start(input), 9, "paragraph should start after '::\\t'");
        }

        /// `::` at end of line — content is on the next line.
        #[test]
        fn content_on_next_line() {
            let input = "- tag ::\n  content\n";
            //           0         1
            //           0123456789012345678
            //                    ^ offset 9 (start of "  content" line)
            assert_eq!(para_start(input), 9, "paragraph should start at beginning of continuation line");
        }
    }
}

mod headline {
    use super::*;

    #[test]
    fn basic_headline() {
        let input = "* \n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_stars() {
        let input = "****  \n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_title() {
        let input = "* title\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_priority() {
        let input = "* TODO [#A] title\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_tags() {
        let input = "* title :tag1:tag2:\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_keyword() {
        let input = "* TODO title\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn multiple_headlines() {
        let input = "* Level 1\n** Level 2\n*** Level 3\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 3, "Expected 3 headlines, found {}", count);
    }

    #[test]
    fn headline_only_stars() {
        let input = "* \n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_tab_after_stars() {
        let input = "*\tOne\n**\tSub\n*\tTwo\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let h1 = root_children.first().expect("Expected first headline");
        if let Syntax::Headline(_) = &arena[*h1].data {
            let h1_end = arena[*h1].location.end;
            let h2 = root_children.get(1).expect("Expected second headline");
            if let Syntax::Headline(_) = &arena[*h2].data {
                assert!(
                    h1_end <= arena[*h2].location.start,
                    "First headline end ({}) should not overlap second headline start ({}): \
                     headlines with tab should form proper subtree boundaries",
                    h1_end,
                    arena[*h2].location.start
                );
            }
        }
    }

    #[test]
    fn headline_with_unicode_tags() {
        let input = "* title :café:标签:\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let h = root_children.first().expect("Expected headline");
        if let Syntax::Headline(data) = &arena[*h].data {
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
        } else {
            panic!("Expected Headline");
        }
    }

    #[test]
    fn headline_comment_not_in_title() {
        let input = "* COMMENT stuff\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let h = root_children.first().expect("Expected headline");
        if let Syntax::Headline(data) = &arena[*h].data {
            assert!(
                !data.title.starts_with("COMMENT"),
                "Title should not include COMMENT keyword, got: {:?}",
                data.title
            );
        } else {
            panic!("Expected Headline");
        }
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
            let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();
            // root → headline (first child)
            let hl = *arena[root].children.first().expect("expected a headline");
            assert_eq!(SyntaxT::from(&arena[hl].data), SyntaxT::Headline);
            arena[hl].children.iter()
                .map(|&id| SyntaxT::from(&arena[id].data))
                .collect()
        }

        /// Plain-text title: `* foo bar` — no PlainText child of Headline.
        #[test]
        fn plain_text_title() {
            let types = headline_direct_child_types("* foo bar\n");
            assert!(
                !types.contains(&SyntaxT::PlainText),
                "PlainText must not be a direct child of Headline; got {types:?}"
            );
        }

        /// Title with bold markup: `* foo *bold* bar` — no Bold or PlainText
        /// child of Headline.
        #[test]
        fn title_with_markup() {
            let types = headline_direct_child_types("* foo *bold* bar\n");
            assert!(
                !types.contains(&SyntaxT::PlainText) && !types.contains(&SyntaxT::Bold),
                "Title markup must not be a direct child of Headline; got {types:?}"
            );
        }

        /// Title with a link: `* See [[url][text]]` — no Link child of Headline.
        #[test]
        fn title_with_link() {
            let types = headline_direct_child_types("* See [[https://example.com][text]]\n");
            assert!(
                !types.contains(&SyntaxT::Link),
                "Link from title must not be a direct child of Headline; got {types:?}"
            );
        }

        /// Headline with a body section: children should only be Section (and
        /// optionally sub-Headlines), never inline objects.
        #[test]
        fn children_are_only_sections_and_headlines() {
            let input = "* Title with *bold*\nsome body text\n** Sub\n";
            let types = headline_direct_child_types(input);
            let inline_types = [
                SyntaxT::PlainText, SyntaxT::Bold, SyntaxT::Italic,
                SyntaxT::Underline, SyntaxT::Code, SyntaxT::Verbatim,
                SyntaxT::Link, SyntaxT::StrikeThrough,
            ];
            for t in inline_types {
                assert!(
                    !types.contains(&t),
                    "{t:?} must not be a direct child of Headline; children: {types:?}"
                );
            }
        }
    }
}

mod table {
    use super::*;

    #[test]
    fn simple_table() {
        let input = "| col 1 | col 2 |\n|-----|-----|\n| a | b |\n| c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_row_count() {
        let input = "| a | b |\n| c | d |\n| e | f |\n";
        let count = get_type_count(input, SyntaxT::TableRow, ParseGranularity::Element);
        assert!(
            count >= 3,
            "Expected at least 3 table rows, found {}",
            count
        );
    }

    #[test]
    fn table_with_header() {
        let input = "| head1 | head2 |\n|--------|--------|\n| body1 | body2 |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_no_hline() {
        let input = "| a | b |\n| c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_indented() {
        let input = "  | a | b |\n  | c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_empty_cells() {
        let input = "| | b |\n| c | |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_eof() {
        let input = "| a | b |";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_hline_detection() {
        let input = "| h1 | h2 |\n|---|\n| c1 | c2 |\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let table = section_children.first().expect("Expected table");
        let table_children = &arena[*table].children;

        let rule_row = table_children
            .get(1)
            .expect("Expected second table row (hline)");
        if let Syntax::TableRow(row_type) = &arena[*rule_row].data {
            assert!(
                matches!(row_type, crate::table::TableRowType::Rule),
                "Expected hline row to be Rule, got {:?}",
                row_type
            );
        } else {
            panic!("Expected TableRow, got: {:?}", arena[*rule_row].data);
        }
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
        assert_eq!(cell_count, 3, "expected 3 TableCell nodes, got {}", cell_count);
    }

    #[test]
    fn table_cell_plain_text_not_direct_child_of_row() {
        // Plain-text inside a table cell must live under the TableCell, not the
        // TableRow.  Rust currently skips the TableCell wrapper and puts objects
        // directly in the row.
        let input = "| foo | bar |\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        fn find_first<'a>(arena: &'a crate::data::NodeArena, id: NodeId, t: SyntaxT)
            -> Option<NodeId>
        {
            if SyntaxT::from(&arena[id].data) == t { return Some(id); }
            arena[id].children.iter().find_map(|&c| find_first(arena, c, t))
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
    fn center() {
        let input = "#+BEGIN_CENTER\nCentered text\n#+END_CENTER\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 center block, found {}", count);
    }

    #[test]
    fn quote() {
        let input = "#+BEGIN_QUOTE\nQuoted text\n#+END_QUOTE\n";
        let count = get_type_count(input, SyntaxT::QuoteBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 quote block, found {}", count);
    }

    #[test]
    fn example() {
        let input = "#+BEGIN_EXAMPLE\nExample text\n#+END_EXAMPLE\n";
        let count = get_type_count(input, SyntaxT::ExampleBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 example block, found {}", count);
    }

    #[test]
    fn src() {
        let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
        let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 src block, found {}", count);
    }

    #[test]
    fn verse() {
        let input = "#+BEGIN_VERSE\nVerse line 1\nVerse line 2\n#+END_VERSE\n";
        let count = get_type_count(input, SyntaxT::VerseBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 verse block, found {}", count);
    }

    #[test]
    fn comment() {
        let input = "#+BEGIN_COMMENT\nComment text\n#+END_COMMENT\n";
        let count = get_type_count(input, SyntaxT::CommentBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment block, found {}", count);
    }

    #[test]
    fn export() {
        let input = "#+BEGIN_EXPORT html\n<div>HTML</div>\n#+END_EXPORT\n";
        let count = get_type_count(input, SyntaxT::ExportBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 export block, found {}", count);
    }

    #[test]
    fn special() {
        let input = "#+BEGIN_SPECIAL\nCustom block content\n#+END_SPECIAL\n";
        let count = get_type_count(input, SyntaxT::SpecialBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 special block, found {}", count);
    }

    #[test]
    fn dynamic() {
        let input = "#+BEGIN: my-block :param value\nBlock content\n#+END:\n";
        let count = get_type_count(input, SyntaxT::DynamicBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 dynamic block, found {}", count);
    }

    #[test]
    fn case_insensitive() {
        let input = "#+begin_center\nCentered\n#+end_center\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 center block (case insensitive), found {}",
            count
        );
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
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let block = section_children.first().expect("Expected export block");

        if let Syntax::ExportBlock(data) = &arena[*block].data {
            assert_eq!(
                data.type_s, "html",
                "Expected export type_s 'html', got {:?}",
                data.type_s
            );
        } else {
            panic!("Expected ExportBlock, got: {:?}", arena[*block].data);
        }
    }

    #[test]
    fn src_block_language() {
        let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let block = section_children.first().expect("Expected src block");

        if let Syntax::SrcBlock(data) = &arena[*block].data {
            assert_eq!(
                data.language,
                Some("python"),
                "Expected language 'python', got {:?}",
                data.language
            );
        } else {
            panic!("Expected SrcBlock, got: {:?}", arena[*block].data);
        }
    }

    #[test]
    fn block_end_edge_cases() {
        let mut parser = Parser::new(
            "#+BEGIN_CENTER\ncenter\n#+end_center \n",
            ParseGranularity::Element,
            DefaultEnvironment,
        );
        let (_arena, _root) = parser.parse_buffer();
        let count = get_type_count(
            "#+BEGIN_CENTER\ncenter\n#+end_center \n",
            SyntaxT::CenterBlock,
            ParseGranularity::Element,
        );
        assert_eq!(
            count, 1,
            "Expected 1 center block with trailing space on END, found {}",
            count
        );
    }
}

mod drawer {
    use super::*;

    #[test]
    fn simple_drawer() {
        let input = ":TEST:\nDrawer content\n:END:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 drawer, found {}", count);
    }

    #[test]
    fn property_drawer() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n";
        let count = get_type_count(input, SyntaxT::PropertyDrawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 property drawer, found {}", count);
    }

    #[test]
    fn drawer_case_insensitive() {
        let input = ":test:\nDrawer content\n:end:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 drawer (case insensitive), found {}",
            count
        );
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

    #[test]
    fn drawer_multiple_properties() {
        let input = ":PROPERTIES:\n:ID: test-id\n:CUSTOM_ID: custom-id\n:END:\n";
        let pd_count = get_type_count(input, SyntaxT::PropertyDrawer, ParseGranularity::Element);
        let prop_count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            pd_count >= 1 && prop_count >= 2,
            "Drawer: {}, Props: {}",
            pd_count,
            prop_count
        );
    }
}

mod planning {
    use super::*;

    #[test]
    fn planning_with_deadline() {
        let input = "* TODO Task\nDEADLINE: <2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn planning_with_scheduled() {
        let input = "* TODO Task\nSCHEDULED: <2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn planning_with_closed() {
        let input = "* TODO Task\nCLOSED: [2023-12-31]\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn planning_all_timestamps() {
        let input =
            "* TODO Task\nDEADLINE: <2023-12-31> SCHEDULED: <2023-12-30> CLOSED: [2023-12-29]\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 planning with all timestamps, found {}",
            count
        );
    }
}

mod comment {
    use super::*;

    #[test]
    fn basic_comment() {
        let input = "# Comment\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment, found {}", count);
    }

    #[test]
    fn comment_indented() {
        let input = "  # Indented comment\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment, found {}", count);
    }

    #[test]
    fn comment_multiple_lines() {
        let input = "# Line 1\n# Line 2\n# Line 3\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment, found {}", count);
    }
}

mod clock {
    use super::*;

    #[test]
    fn running_clock() {
        let input = "CLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock, found {}", count);
    }

    #[test]
    fn closed_clock() {
        let input = "CLOCK: [2023-10-13 Fri 14:40]--[2023-10-13 Fri 14:51] => 0:11\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock, found {}", count);
    }

    #[test]
    fn clock_case_insensitive() {
        let input = "Clock: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn clock_duration_only() {
        let input = "CLOCK: => 0:11\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock with duration only, found {}",
            count
        );
    }

    #[test]
    fn must_be_bol() {
        let input = "   CLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock (whitespace allowed per Elisp), found {}",
            count
        );
    }

    #[test]
    fn with_leading_whitespace() {
        let input = "\tCLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock with tab, found {}", count);
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
    fn basic() {
        let input = "%%(org-anniversary 1956 5 14) Arthur Dent is %d years old\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 diary sexp, found {}", count);
    }

    #[test]
    fn simple() {
        let input = "%%(diary-sexp)\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 diary sexp, found {}", count);
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
    fn basic() {
        let input = "\\begin{equation}\nE = mc^2\n\\end{equation}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex environment, found {}", count);
    }

    #[test]
    fn latex_equation_environment() {
        let input = "\\begin{equation}\n\\frac{a}{b}\n\\end{equation}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex equation env, found {}", count);
    }

    #[test]
    fn latex_aligned_environment() {
        let input = "\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex aligned env, found {}", count);
    }
}

mod footnote_definition {
    use super::*;

    #[test]
    fn footnote_definition() {
        let input = "[fn:1] This is a footnote.\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition, found {}", count);
    }

    #[test]
    fn named_footnote_definition() {
        let input = "[fn:my-note] This is a named footnote.\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 named footnote, found {}", count);
    }

    #[test]
    fn multiline() {
        let input = "[fn:1]\nMulti-line\nfootnote content\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 multiline footnote, found {}", count);
    }

    #[test]
    fn content_location() {
        let input = "[fn:1] This is footnote content\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition");

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected at least one child");
        let section_children = &arena[*section].children;
        let fn_node = section_children
            .first()
            .expect("Expected footnote in section");

        if let Syntax::FootnoteDefinition(_) = &arena[*fn_node].data {
            let content_loc = &arena[*fn_node]
                .content_location
                .expect("Footnote definition should have content_location");

            assert_eq!(
                content_loc.start, 7,
                "contents-begin should be after [fn:1] and space"
            );

            let content = &input[content_loc.start..content_loc.end];
            assert_eq!(content, "This is footnote content", "content should match");
        } else {
            panic!("Expected FootnoteDefinition, got: {:?}", arena[*fn_node].data);
        }
    }

    #[test]
    fn with_label() {
        let input = "[fn:my-label] Some content\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition");

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected at least one child");
        let section_children = &arena[*section].children;
        let fn_node = section_children
            .first()
            .expect("Expected footnote in section");

        if let Syntax::FootnoteDefinition(data) = &arena[*fn_node].data {
            assert_eq!(data.label, "my-label", "Label should be extracted");
            assert_eq!(data.value, "Some content", "Value should be raw text");
        } else {
            panic!("Expected FootnoteDefinition syntax type");
        }
    }

    #[test]
    fn footnote_reference_inline() {
        let input = "Paragraph with reference[fn:1] and another[fn:inline:inline note]\n";
        let count = get_type_count(input, SyntaxT::FootnoteReference, ParseGranularity::Object);
        assert!(count >= 2, "Expected footnotes, found {}", count);
    }
}

mod fixed_width {
    use super::*;
    use crate::markup::strip_fixed_width_colons;

    #[test]
    fn basic() {
        let input = ": Fixed width line\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 fixed width, found {}", count);
    }

    #[test]
    fn with_name_affiliated() {
        let input = "#+NAME: my-fixed\n: Fixed width line\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 fixed width with name, found {}",
            count
        );
    }

    #[test]
    fn multiple_lines() {
        let input = ": Line 1\n: Line 2\n: Line 3\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 fixed width region, found {}", count);
    }

    #[test]
    fn multiline_value_strips_colons() {
        let input = ": Line 1\n: Line 2\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let fw_node = section_children.first().expect("Expected fixed-width node");

        if let Syntax::FixedWidth(raw) = &arena[*fw_node].data {
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
        } else {
            panic!("Expected FixedWidth, got: {:?}", arena[*fw_node].data);
        }
    }

    #[test]
    fn indented() {
        let input = "  : Indented fixed width\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 indented fixed width, found {}", count);
    }
}

mod babel_call {
    use super::*;

    #[test]
    fn babel_call() {
        let input = "#+CALL: test()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 babel call, found {}", count);
    }

    #[test]
    fn case_insensitive() {
        let input = "#+call: test()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn with_header() {
        let input = "#+CALL: test[:results output]()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call with header, found {}",
            count
        );
    }

    #[test]
    fn with_arguments() {
        let input = "#+CALL: test(n=4)\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call with arguments, found {}",
            count
        );
    }
}

mod node_properties {
    use super::*;

    #[test]
    fn basic() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            count >= 1,
            "Expected at least 1 node property, found {}",
            count
        );
    }

    #[test]
    fn multiple_node_properties() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:PRIORITY: A\n: tags: :foo:bar:\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            count >= 3,
            "Expected at least 3 node properties, found {}",
            count
        );
    }

    /// `:END:` closes the drawer and must not be parsed as a `NodeProperty`
    /// with key `"END"`.  Regression test: the content range previously
    /// included the `:END:` line, causing `parse_node_property_line` to
    /// emit a spurious extra property.
    #[test]
    fn end_marker_not_a_property() {
        let input = ":PROPERTIES:\n:ID: abc123\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert_eq!(count, 1, ":END: must not be parsed as a node property (got {count})");
    }
}

mod keyword {
    use super::*;

    #[test]
    fn keyword() {
        let input = "#+KEYWORD: value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }

    #[test]
    fn case_insensitive() {
        let input = "#+keyword: value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 keyword (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn keyword_with_space() {
        let input = "#+KEYWORD:    spaced value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }

    #[test]
    fn keyword_with_newline() {
        let input = "#+KEYWORD: value\nparagraph\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }

    #[test]
    fn keyword_edge_cases() {
        let mut parser = Parser::new(
            "#+KEY: val\n#+EMPTY:\n#+COLONS: a::b::c\n#+UNICODE: café 标签\n",
            ParseGranularity::Element,
            DefaultEnvironment,
        );
        let (_arena, _root) = parser.parse_buffer();
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
    fn horizontal_rule() {
        let input = "-----\n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 horizontal rule, found {}", count);
    }

    #[test]
    fn with_spaces() {
        let input = "   -----   \n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 horizontal rule with spaces, found {}",
            count
        );
    }
}

mod inlinetask {
    use super::*;

    #[test]
    fn basic() {
        let input = "*************** Inlinetask content\n";
        let count = get_type_count(input, SyntaxT::InlineTask, ParseGranularity::Element);
        let headline_count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert!(
            headline_count >= 1 || count >= 1,
            "Expected headline or inlinetask, found headline: {}, inlinetask: {}",
            headline_count,
            count
        );
    }

    #[test]
    fn fifteen_stars_is_not_headline() {
        let input = "*************** Inlinetask\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
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
    fn bold_object() {
        let input = "*bold*\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 bold object, found {}", count);
    }

    #[test]
    fn italic_object() {
        let input = "/italic/\n";
        let count = get_type_count(input, SyntaxT::Italic, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 italic object, found {}", count);
    }

    #[test]
    fn underline_object() {
        let input = "_underline_\n";
        let count = get_type_count(input, SyntaxT::Underline, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 underline object, found {}", count);
    }

    #[test]
    fn strikethrough_object() {
        let input = "+strikethrough+\n";
        let count = get_type_count(input, SyntaxT::StrikeThrough, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 strikethrough object, found {}", count);
    }

    #[test]
    fn code_object() {
        let input = "~code~\n";
        let count = get_type_count(input, SyntaxT::Code, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 code object, found {}", count);
    }

    #[test]
    fn verbatim_object() {
        let input = "=verbatim=\n";
        let count = get_type_count(input, SyntaxT::Verbatim, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 verbatim object, found {}", count);
    }

    #[test]
    fn entity_object() {
        let input = "\\alpha\n";
        let count = get_type_count(input, SyntaxT::Entity, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 entity object, found {}", count);
    }

    #[test]
    fn link_object() {
        let input = "[[https://example.com][link]]\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 link object, found {}", count);
    }

    #[test]
    fn target_object() {
        let input = "<<target>>\n";
        let count = get_type_count(input, SyntaxT::Target, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 target object, found {}", count);
    }

    #[test]
    fn timestamp_object() {
        let input = "<2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Timestamp, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 timestamp object, found {}", count);
    }

    #[test]
    fn single_char_bold() {
        let input = "*a*\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert_eq!(
            count, 1,
            "Expected 1 bold from single-char '*a*', found {}",
            count
        );
    }

    #[test]
    fn single_char_italic() {
        let input = "/a/\n";
        let count = get_type_count(input, SyntaxT::Italic, ParseGranularity::Object);
        assert_eq!(
            count, 1,
            "Expected 1 italic from single-char '/a/', found {}",
            count
        );
    }

    #[test]
    fn link_after_punctuation() {
        let input = "[[link]].\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 link before '.', found {}", count);
    }

    #[test]
    fn target_after_punctuation() {
        let input = "<<target>>.\n";
        let count = get_type_count(input, SyntaxT::Target, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 target before '.', found {}", count);
    }

    #[test]
    fn timestamp_after_punctuation() {
        let input = "<2023-12-31>.\n";
        let count = get_type_count(input, SyntaxT::Timestamp, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 timestamp before '.', found {}", count);
    }

    #[test]
    fn timestamp_with_time_range() {
        let input = "<2023-12-01 10:15-11:30>\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();

        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let para = section_children.first().expect("Expected paragraph");
        let para_children = &arena[*para].children;
        let ts_node = para_children.first().expect("Expected timestamp");

        if let Syntax::Timestamp(data) = &arena[*ts_node].data {
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
        } else {
            panic!("Expected Timestamp, got: {:?}", arena[*ts_node].data);
        }
    }

    #[test]
    fn link_type_with_brackets() {
        use crate::data::LinkType;
        let input = "[[https://example.com]]\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("Expected section");
        let section_children = &arena[*section].children;
        let para = section_children.first().expect("Expected paragraph");
        let para_children = &arena[*para].children;
        let link_node = para_children.first().expect("Expected link");
        if let Syntax::Link(data) = &arena[*link_node].data {
            assert!(
                matches!(data.link_type(), LinkType::File),
                "Expected LinkType::File for https link, got {:?}",
                data.link_type()
            );
        } else {
            panic!("Expected Link, got: {:?}", arena[*link_node].data);
        }
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
        assert!(
            list_count >= 1,
            "Expected at least 1 list when '- item' is mid-paragraph, found {}",
            list_count
        );
    }
}

mod granularity {
    use super::*;

    #[test]
    fn headline_granularity() {
        let input = "* Headline\n\nSome paragraph\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Headline);
        assert!(
            count >= 1,
            "Expected at least 1 headline at Headline granularity, found {}",
            count
        );
    }

    #[test]
    fn greater_element_granularity() {
        let input = "* Headline\n\n#+BEGIN_CENTER\nCenter\n#+END_CENTER\n";
        let count = get_type_count(
            input,
            SyntaxT::CenterBlock,
            ParseGranularity::GreaterElement,
        );
        assert!(
            count >= 1,
            "Expected center block at GreaterElement granularity, found {}",
            count
        );
    }

    #[test]
    fn element_granularity() {
        let input = "* Headline\n\nParagraph text\n";
        let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert!(
            count >= 1,
            "Expected paragraph at Element granularity, found {}",
            count
        );
    }

    #[test]
    fn object_granularity() {
        let input = "Paragraph with *bold* text\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold at Object granularity, found {}",
            count
        );
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
        let bold = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        let italic = get_type_count(input, SyntaxT::Italic, ParseGranularity::Object);
        let code = get_type_count(input, SyntaxT::Code, ParseGranularity::Object);
        let verbatim = get_type_count(input, SyntaxT::Verbatim, ParseGranularity::Object);
        assert_eq!(bold, 1, "Expected 1 bold");
        assert_eq!(italic, 1, "Expected 1 italic");
        assert_eq!(code, 1, "Expected 1 code");
        assert_eq!(verbatim, 1, "Expected 1 verbatim");
    }

    #[test]
    fn heading_levels_1_to_3() {
        let input = "* Level 1\n** Level 2\n*** Level 3\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(
            count, 3,
            "Expected headlines at levels 1, 2 and 3, found {}",
            count
        );
    }

    #[test]
    fn description_list_type() {
        use crate::list::ListKind;
        let input = "- term :: description text\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section node");
        let section_ch = &arena[*section].children;
        let list = section_ch.first().expect("list node");
        if let Syntax::PlainList(data) = &arena[*list].data {
            assert!(
                matches!(data.type_s, ListKind::Descriptive),
                "Expected Descriptive list type, got {:?}",
                data.type_s
            );
        } else {
            panic!("Expected PlainList, got {:?}", arena[*list].data);
        }
    }

    #[test]
    fn checkbox_checked() {
        let input = "- [X] done item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 item with [X] checkbox, found {}",
            count
        );
    }

    #[test]
    fn checkbox_unchecked() {
        let input = "- [ ] todo item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 item with [ ] checkbox, found {}",
            count
        );
    }

    #[test]
    fn checkbox_in_progress() {
        let input = "- [-] in-progress item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 item with [-] checkbox, found {}",
            count
        );
    }

    #[test]
    fn src_block_with_language() {
        let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section");
        let section_ch = &arena[*section].children;
        let block = section_ch.first().expect("src block");
        if let Syntax::SrcBlock(data) = &arena[*block].data {
            assert_eq!(
                data.language,
                Some("python"),
                "Expected language 'python', got {:?}",
                data.language
            );
        } else {
            panic!("Expected SrcBlock, got {:?}", arena[*block].data);
        }
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
    fn link_bare_https_url() {
        let input = "Visit https://example.com today\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bare https URL parsed as link, found {}",
            count
        );
    }

    #[test]
    fn link_bracketed_no_description() {
        let input = "[[https://example.com]]\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert_eq!(
            count, 1,
            "Expected 1 bracketed link without description, found {}",
            count
        );
    }

    #[test]
    fn link_bracketed_with_description() {
        let input = "[[https://example.com][visit here]]\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert_eq!(
            count, 1,
            "Expected 1 bracketed link with description, found {}",
            count
        );
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
            let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
            let (arena, root) = parser.parse_buffer();
            // Walk to the Link node
            fn find_link<'a>(arena: &'a crate::data::NodeArena<'a>, id: NodeId) -> Option<NodeId> {
                if SyntaxT::from(&arena[id].data) == SyntaxT::Link { return Some(id); }
                for &child in &arena[id].children { if let Some(l) = find_link(arena, child) { return Some(l); } }
                None
            }
            let link = find_link(&arena, root).expect("no Link found");
            arena[link].children.iter().map(|&id| SyntaxT::from(&arena[id].data)).collect()
        }

        /// Plain-text description becomes a `PlainText` child of the link.
        #[test]
        fn plain_description_is_child() {
            let children = link_children("[[https://example.com][OpenPGP]]\n");
            assert!(
                children.contains(&SyntaxT::PlainText),
                "expected PlainText child for link description; got {children:?}"
            );
        }

        /// A link without a description has no children.
        #[test]
        fn no_description_no_children() {
            let children = link_children("[[https://example.com]]\n");
            assert!(
                children.is_empty(),
                "link without description should have no children; got {children:?}"
            );
        }

        /// A description containing bold markup yields a `Bold` child.
        #[test]
        fn bold_description_is_child() {
            let children = link_children("[[https://example.com][*bold*]]\n");
            assert!(
                children.contains(&SyntaxT::Bold),
                "expected Bold child for bold link description; got {children:?}"
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
    fn horizontal_rule_five_dashes() {
        let input = "-----\n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 horizontal rule, found {}", count);
    }

    #[test]
    fn comment_indented() {
        let input = "  # indented comment\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 indented comment, found {}", count);
    }

    #[test]
    fn description_list_tag_is_term() {
        let input = "- term :: description text\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section");
        let section_ch = &arena[*section].children;
        let list = section_ch.first().expect("list");
        if let Syntax::PlainList(data) = &arena[*list].data {
            let tag = &data.structure.items[0].tag;
            assert_eq!(
                *tag,
                Some("term"),
                "Item tag should be the term before '::' (got {:?})",
                tag
            );
        } else {
            panic!("Expected PlainList");
        }
    }

    #[test]
    fn nested_list_inner_list_parsed() {
        let input = "- outer\n  - inner\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert!(
            count >= 2,
            "Expected outer and inner PlainList, found {}",
            count
        );
    }

    #[test]
    fn bold_in_list_item_parsed() {
        let input = "- *bold* item\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold object inside list item, found {}",
            count
        );
    }

    #[test]
    fn bold_in_table_cell_parsed() {
        let input = "| *bold* text |\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold object inside table cell, found {}",
            count
        );
    }

    #[test]
    fn bold_in_quote_block_parsed() {
        let input = "#+BEGIN_QUOTE\n*bold*\n#+END_QUOTE\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold object inside quote block content, found {}",
            count
        );
    }

    #[test]
    fn bold_in_verse_block_parsed() {
        let input = "#+BEGIN_VERSE\n*bold*\n#+END_VERSE\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold object inside verse block content, found {}",
            count
        );
    }

    #[test]
    fn bold_in_center_block_parsed() {
        let input = "#+BEGIN_CENTER\n*bold*\n#+END_CENTER\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold object inside center block content, found {}",
            count
        );
    }

    #[test]
    fn headline_priority_extracted() {
        let input = "* [#A] important task\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let headline = root_children.first().expect("headline");
        if let Syntax::Headline(data) = &arena[*headline].data {
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
        } else {
            panic!("Expected Headline, got {:?}", arena[*headline].data);
        }
    }

    #[test]
    fn block_end_must_match_block_type() {
        // #+END_SRC on its own line must not close a #+BEGIN_QUOTE block.
        // With the bug, the quote closes early and "more" / "#+END_QUOTE"
        // become stray section-level paragraphs.
        let input = "#+BEGIN_QUOTE\n#+END_SRC\nmore\n#+END_QUOTE\n";
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let root_children = &arena[root].children;
        let section = root_children.first().expect("section");
        let para_count = arena[*section].children
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
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let hl = *arena[root].children.first().expect("expected headline");
        let crate::data::Syntax::Headline(ref data) = arena[hl].data else {
            panic!("expected Headline");
        };
        let has_bold = data.title_objects.iter().any(|&id| {
            SyntaxT::from(&arena[id].data) == SyntaxT::Bold
        });
        assert!(has_bold, "expected Bold in title_objects; got {:?}",
            data.title_objects.iter().map(|&id| SyntaxT::from(&arena[id].data)).collect::<Vec<_>>());
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
        assert!(
            count >= 1,
            "Expected bold inside footnote inline definition, found {}",
            count
        );
    }

    #[test]
    fn timestamp_with_dayname_and_time() {
        let input = "<2023-12-31 Sun 10:30>\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
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
        if let Syntax::Timestamp(data) = &arena[*ts].data {
            assert_eq!(
                data.hour_start,
                Some(10),
                "Expected hour 10 from '<2023-12-31 Sun 10:30>', got {:?}",
                data.hour_start
            );
        } else {
            panic!("Expected Timestamp");
        }
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
        let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
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
        if let Syntax::Planning(p) = &arena[*planning].data {
            assert!(
                p.deadline.is_some(),
                "DEADLINE should be parsed even when CLOSED follows on the same line"
            );
        }
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
    fn bare_subscript() {
        let input = "H_2\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let scripts = find_script(&arena, root);
        assert!(!scripts.is_empty(), "expected a Script node for 'H_2'; got none");
        let kind = if let Syntax::Script(f) = arena[scripts[0]].data { f.kind() } else { unreachable!() };
        assert_eq!(kind, ScriptKind::Sub, "expected Sub, got {:?}", kind);
    }

    #[test]
    fn braced_subscript() {
        let input = "H_{2}O\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let scripts = find_script(&arena, root);
        assert!(!scripts.is_empty(), "expected a Script node for 'H_{{2}}O'; got none");
    }

    #[test]
    fn bare_superscript() {
        let input = "E=mc^2\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let scripts = find_script(&arena, root);
        assert!(!scripts.is_empty(), "expected a Script node for 'mc^2'; got none");
        let kind = if let Syntax::Script(f) = arena[scripts[0]].data { f.kind() } else { unreachable!() };
        assert_eq!(kind, ScriptKind::Sup, "expected Sup, got {:?}", kind);
    }

    #[test]
    fn braced_superscript() {
        let input = "x^{n+1}\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let scripts = find_script(&arena, root);
        assert!(!scripts.is_empty(), "expected a Script node for 'x^{{n+1}}'; got none");
    }

    #[test]
    fn bare_subscript_has_plain_text_child() {
        // "x_foo test\n"
        //  0 1   5
        // Emacs: subscript at [1,6) with plain-text child "foo" at [2,5).
        // Rust currently emits a Script node with empty children.
        let input = "x_foo test\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let scripts = find_script(&arena, root);
        assert!(!scripts.is_empty(), "no Script node found");
        let script_id = scripts[0];
        let children = &arena[script_id].children;
        assert_eq!(children.len(), 1, "Script must have exactly 1 PlainText child, got {}", children.len());
        assert!(
            matches!(arena[children[0]].data, Syntax::PlainText(_)),
            "Script child must be PlainText"
        );
    }

    #[test]
    fn braced_subscript_has_plain_text_child() {
        // "H_{2}O\n" — braced subscript {2} must contain a PlainText child.
        let input = "H_{2}O\n";
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
        let (arena, root) = parser.parse_buffer();
        let scripts = find_script(&arena, root);
        assert!(!scripts.is_empty(), "no Script node found");
        let script_id = scripts[0];
        let children = &arena[script_id].children;
        assert_eq!(children.len(), 1, "Script must have exactly 1 PlainText child, got {}", children.len());
        assert!(
            matches!(arena[children[0]].data, Syntax::PlainText(_)),
            "Script child must be PlainText"
        );
    }
}

mod post_blank {
    use super::*;

    fn plain_text_starts(input: &str) -> Vec<usize> {
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment);
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
    fn plain_text_after_bold_starts_after_space() {
        // "text *bold* word\n"
        //  0    5    10 11 12
        //               ^  ^-- 'w': Emacs plain_text begin
        //               ' ': current Rust plain_text begin (wrong)
        let starts = plain_text_starts("text *bold* word\n");
        assert!(starts.contains(&12),
            "expected plain_text at 12 ('w'), got starts: {:?}", starts);
        assert!(!starts.contains(&11),
            "plain_text must not start at 11 (the post-blank space), got starts: {:?}", starts);
    }

    #[test]
    fn plain_text_after_code_starts_after_space() {
        // "see ~foo~ bar\n"
        //  0   4   8 9 10
        //              ^-- 'b': Emacs plain_text begin
        let starts = plain_text_starts("see ~foo~ bar\n");
        assert!(starts.contains(&10),
            "expected plain_text at 10 ('b'), got starts: {:?}", starts);
        assert!(!starts.contains(&9),
            "plain_text must not start at 9 (post-blank space), got starts: {:?}", starts);
    }

    #[test]
    fn no_space_after_bold_unaffected() {
        // "a *bold*.\n" — post-char is '.', not a space; no post_blank to skip
        //  0 2      89
        //           ^-- '.': plain_text starts immediately after bold
        let starts = plain_text_starts("a *bold*.\n");
        assert!(starts.contains(&8),
            "expected plain_text at 8 ('.'), got starts: {:?}", starts);
    }

    #[test]
    fn failed_strikethrough_does_not_split_plain_text() {
        // "text + more\n"
        //  0    5
        //       ^-- '+': pre-char (' ') triggers scan_plain_text_end stop here,
        //            try_parse_strikethrough fails (no closing '+'), but Rust
        //            currently creates a second PlainText starting at 5.
        // Emacs emits a single PlainText for the whole "text + more\n".
        let starts = plain_text_starts("text + more\n");
        assert!(
            !starts.contains(&5),
            "plain_text must not start at 5 (the bare '+'); got starts: {:?}", starts
        );
    }

    #[test]
    fn failed_italic_at_line_start_does_not_split_plain_text() {
        // "foo\n/bar\n"
        //  0   3 4
        //        ^-- '/': pre-char ('\n') triggers stop, try_parse_italic fails
        //             (no closing '/'), but Rust currently creates a second
        //             PlainText starting at 4.
        // Emacs emits a single PlainText for the whole "foo\n/bar\n".
        let starts = plain_text_starts("foo\n/bar\n");
        assert!(
            !starts.contains(&4),
            "plain_text must not start at 4 (the bare '/'); got starts: {:?}", starts
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
        let input = "[[help:consult-outline][consult-outline]] / [[help:consult-imenu][consult-imenu]].\n";
        let starts = plain_text_starts(input);
        assert!(
            starts.contains(&42),
            "expected plain_text at 42 ('/'), got starts: {:?}", starts
        );
        assert!(
            !starts.contains(&41),
            "plain_text must not start at 41 (post-blank space after link), got starts: {:?}", starts
        );
    }
}
