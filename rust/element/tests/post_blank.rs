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
use bumpalo::Bump;
use common::*;
use org_element::data::{NodeArena, NodeId, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

fn plain_text_starts(input: &str) -> Vec<usize> {
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let mut starts = Vec::new();
    collect_plain_text_starts(&arena, root, &mut starts);
    starts
}

fn collect_plain_text_starts(
    arena: &org_element::data::NodeArena,
    id: NodeId,
    out: &mut Vec<usize>,
) {
    if matches!(arena[id].data, Syntax::PlainText(_)) {
        out.push(arena[id].location.start);
    }
    for &child in &arena[id].children {
        collect_plain_text_starts(arena, child, out);
    }
}

#[test]
fn plain_text_after_markup_starts_after_space() {
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
    let starts = plain_text_starts("a *bold*.\n");
    assert!(
        starts.contains(&8),
        "expected plain_text at 8 ('.'), got starts: {:?}",
        starts
    );
}

#[test]
fn failed_markup_does_not_split_plain_text() {
    let starts = plain_text_starts("text + more\n");
    assert!(
        !starts.contains(&5),
        "plain_text must not start at 5 (the bare '+'); got starts: {:?}",
        starts
    );

    let starts = plain_text_starts("foo\n/bar\n");
    assert!(
        !starts.contains(&4),
        "plain_text must not start at 4 (the bare '/'); got starts: {:?}",
        starts
    );
}

#[test]
fn plain_text_after_bracket_link_starts_after_space() {
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
    let input = "https://example.com  \nrest\n";
    let starts = plain_text_starts(input);
    assert!(
        !starts.contains(&19),
        "PlainText must not start at byte 19 (first trailing space); spaces must be absorbed by plain link. starts: {:?}",
        starts
    );
}

#[test]
fn plain_link_before_headline() {
    let input = "text http://orgmode.org\n* headline\n";
    let starts = plain_text_starts(input);
    assert!(
        starts.contains(&23),
        "expected PlainText at byte 23 ('\\n' after URL), got starts: {:?} — \
         the bare URL is being left as PlainText instead of parsed as a Link",
        starts
    );
    assert!(
        !starts.contains(&5),
        "PlainText must not start at byte 5 ('h' of http://); \
         http://orgmode.org should be parsed as a Link, got starts: {:?}",
        starts
    );
}

#[test]
fn plain_link_after_non_ascii_pre_char() {
    let starts = plain_text_starts("中http://example.org more\n");
    assert!(
        starts.contains(&22),
        "expected PlainText at byte 22 ('m' of 'more'), got starts: {:?} — \
         http:// after a CJK char is being left as PlainText",
        starts
    );
    assert!(
        !starts.contains(&3),
        "PlainText must not start at byte 3 ('h' of http://); \
         the bare URL after non-ASCII text should be parsed as a Link, \
         got starts: {:?}",
        starts
    );
}

#[test]
fn plain_text_after_timestamp_starts_after_space() {
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
    let starts = plain_text_starts("[2021-05-05 Wed]   Discussion\n");
    assert!(
        starts.contains(&19),
        "expected plain_text at 19 ('D'), got starts: {:?}",
        starts
    );
}

#[test]
fn plain_text_after_timestamp_no_space() {
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
    let starts = plain_text_starts(r"\Office16\OUTLOOK rest\n");
    assert!(
        starts.contains(&7),
        "expected PlainText '16' at byte 7 (LaTeX command must stop before digits), got starts: {:?}",
        starts
    );
}

#[test]
fn latex_inline_math_dollar() {
    let starts = plain_text_starts("$E=mc^2$ rest\n");
    assert!(
        starts.contains(&9),
        "expected PlainText at byte 9 ('rest'), got starts: {:?}",
        starts
    );
    assert!(
        !starts.contains(&0),
        "expected no PlainText at 0 — $...$ LaTeX math should be parsed, got starts: {:?}",
        starts
    );
}

#[test]
fn latex_display_math_dollar() {
    let starts = plain_text_starts("$$E=mc^2$$ rest\n");
    assert!(
        starts.contains(&11),
        "expected PlainText at byte 11 ('rest'), got starts: {:?}",
        starts
    );
    assert!(
        !starts.contains(&0),
        "expected no PlainText at 0 — $$...$$ LaTeX math should be parsed, got starts: {:?}",
        starts
    );
}

#[test]
fn latex_inline_math_paren() {
    let starts = plain_text_starts(r"\(E=mc^2\) rest\n");
    assert!(
        starts.contains(&11),
        "expected PlainText at byte 11 ('rest'), got starts: {:?}",
        starts
    );
    assert!(
        !starts.contains(&0),
        r"expected no PlainText at 0 — \(...\) LaTeX math should be parsed, got starts: {:?}",
        starts
    );
}

#[test]
fn latex_display_math_bracket() {
    let starts = plain_text_starts(r"\[E=mc^2\] rest\n");
    assert!(
        starts.contains(&11),
        "expected PlainText at byte 11 ('rest'), got starts: {:?}",
        starts
    );
    assert!(
        !starts.contains(&0),
        r"expected no PlainText at 0 — \[...\] LaTeX math should be parsed, got starts: {:?}",
        starts
    );
}

#[test]
fn statistics_cookie_in_link_description() {
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
    // Verified against Emacs oracle: b (indent 2) and c (indent 1) each get
    // their own PlainList as siblings inside item a (3 PlainLists total).
    let input = "- a\n  - b\n - c\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let pl_count = count_type(&arena, root, SyntaxT::PlainList);
    assert_eq!(
        pl_count, 3,
        "Expected 3 PlainLists (outer + PlainList_b + PlainList_c), got {}",
        pl_count
    );
    let sec = arena[root].children[0];
    let pl = arena[sec].children[0];
    let item_a = arena[pl].children[0];
    // item_a must have exactly 2 PlainList children (one for b, one for c)
    let inner_lists: Vec<_> = arena[item_a]
        .children
        .iter()
        .filter(|&&id| matches!(arena[id].data, Syntax::PlainList(_)))
        .copied()
        .collect();
    assert_eq!(
        inner_lists.len(),
        2,
        "item 'a' should have 2 PlainList children (b and c separate), got {}",
        inner_lists.len()
    );
}

/// Same issue with `*` bullets and tab-derived indentation.  Items at
/// indent 11 followed by indent 10 should each get their own PlainList.
///
/// Verified against Emacs oracle: correct structure (4 PlainLists):
///   (plain-list          ← outer: item a
///     (item "a"
///       (plain-list      ← PlainList_b: item b (indent 11)
///         (item "b"))
///       (plain-list      ← PlainList_c: item c (indent 10)
///         (item "c"
///           (plain-list  ← PlainList_d: item d (indent 11, inside c)
///             (item "d"))))))
#[test]
fn star_list_indent_goes_up_then_down() {
    let input = "\t* a\n   \t* b\n\t  * c\n   \t* d\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let pl_count = count_type(&arena, root, SyntaxT::PlainList);
    assert_eq!(
        pl_count, 4,
        "Expected 4 PlainLists (outer + PlainList_b + PlainList_c + PlainList_d), got {}",
        pl_count
    );
    let sec = arena[root].children[0];
    let outer_pl = arena[sec].children[0];
    let item_a = arena[outer_pl].children[0];
    // item_a must have 2 PlainList children: one for b, one for c
    let a_lists: Vec<_> = arena[item_a]
        .children
        .iter()
        .filter(|&&id| matches!(arena[id].data, Syntax::PlainList(_)))
        .copied()
        .collect();
    assert_eq!(
        a_lists.len(),
        2,
        "item 'a' should have 2 PlainList children, got {}",
        a_lists.len()
    );
    // item c is in the second PlainList under a; it must contain PlainList_d
    let pl_c = a_lists[1];
    let item_c = arena[pl_c].children[0];
    let c_has_sub = arena[item_c]
        .children
        .iter()
        .any(|&id| matches!(arena[id].data, Syntax::PlainList(_)));
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
