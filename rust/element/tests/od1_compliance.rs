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

use bumpalo::Bump;
use org_element::data::{NodeId, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
/// Tests verifying coverage of the 21 Orgdown Level 1 (OD-1) syntax elements.
/// Reference: https://gitlab.com/publicvoit/orgdown/-/blob/master/doc/Orgdown1-Syntax-Examples.org
use org_element::testutils::*;

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
    use org_element::list::ListKind;
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
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        // Walk to the Link node
        fn find_link(arena: &org_element::data::NodeArena<'_, '_>, id: NodeId) -> Option<NodeId> {
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
    let org_element::data::Syntax::Headline(ref data) = arena[hl].data else {
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
