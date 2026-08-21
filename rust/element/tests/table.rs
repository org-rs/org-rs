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

use org_element::data::SyntaxT;
use org_element::testutils::*;

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
        matches!(row_type, org_element::table::TableRowType::Rule),
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
        arena: &org_element::data::NodeArena<'_, '_>,
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
    let keyword_count = section_children
        .iter()
        .filter(|&&id| matches!(&arena[id].data, Syntax::Keyword(_)))
        .count();

    assert_eq!(
        table_count, 1,
        "Expected 1 Table node (Emacs absorbs #+TBLFM: into Table), found {} Keyword",
        keyword_count
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

// ── Citation in table cell ─────────────────────────────────────────
// 2021-07-31-citations.org: `try_parse_citation` (parser.rs:944-949)
// consumes `[cite/...]` bytes unconditionally.  When
// `TableCell.can_contain(Citation)` returns false, the node is
// dropped but bytes are already consumed — no PlainText fallback.
//
// Fix: gate behind `restriction(SyntaxT::Citation)` like StatisticsCookie.
#[test]
fn citation_bytes_consumed_in_table_cell() {
    let input = "| [cite/t/f:@OrgCitations] |\n";
    let cite_count = get_type_count(input, SyntaxT::Citation, ParseGranularity::Object);
    let para_count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
    assert!(
        cite_count > 0 || para_count > 0,
        "Citation bytes vanish in table cell: cite={}, para={}",
        cite_count,
        para_count
    );
}
