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

mod common;
use common::*;

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
        if SyntaxT::from(&arena[id].data) == SyntaxT::Paragraph && arena[id].location.start > 10 {
            *target = arena[id].location.start;
        }
        for &c in &arena[id].children {
            find_inner_para(arena, c, target);
        }
    }
    find_inner_para(&arena, root, &mut inner_para_start);
    assert!(inner_para_start > 0, "Should find an inner paragraph");
    // Expected: content starts at byte 12 (`:term VALUE`)
    // Bug puts it at byte 19 (`A` in `VALUE`)
    assert_eq!(
        inner_para_start, 12,
        "Inner paragraph should start at byte 12 (:term VALUE), got {inner_para_start}"
    );
}
