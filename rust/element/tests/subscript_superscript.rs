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
mod common;
use bumpalo::Bump;
use common::*;
use org_element::data::{NodeArena, NodeId, ScriptKind, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

fn find_script(arena: &org_element::data::NodeArena, id: NodeId) -> Vec<NodeId> {
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
        (
            "?^字母\n",
            Some(ScriptKind::Sup),
            "superscript with CJK chars",
        ),
        (
            "x^αβγ\n",
            Some(ScriptKind::Sup),
            "superscript with Greek letters",
        ),
    ] {
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
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
    for (input, desc) in &[("x_foo test\n", "bare"), ("H_{2}O\n", "braced")] {
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
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

#[test]
fn dollar_in_table_formula_is_latex_fragment() {
    let input = ",$>.其中$<固定表示第一列,$>固定表示最后一列\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    // Print all nodes for debugging
    use std::fmt::Write;
    fn dump(arena: &NodeArena, id: NodeId, indent: usize, out: &mut String) {
        let node = &arena[id];
        let loc = node.location;
        match &node.data {
            Syntax::LatexFragment(v) => {
                writeln!(
                    out,
                    "{:indent$}LatexFragment({:?}) {}..{}",
                    "",
                    v,
                    loc.start,
                    loc.end,
                    indent = indent
                )
                .unwrap();
            }
            Syntax::PlainText(v) => {
                let text: String = v.chars().take(30).collect();
                writeln!(
                    out,
                    "{:indent$}PlainText({:?}) {}..{}",
                    "",
                    text,
                    loc.start,
                    loc.end,
                    indent = indent
                )
                .unwrap();
            }
            Syntax::Script(_) => {
                writeln!(
                    out,
                    "{:indent$}Script {}..{}",
                    "",
                    loc.start,
                    loc.end,
                    indent = indent
                )
                .unwrap();
            }
            _ => {
                writeln!(
                    out,
                    "{:indent$}{:?} {}..{}",
                    "",
                    node.data,
                    loc.start,
                    loc.end,
                    indent = indent
                )
                .unwrap();
            }
        }
        for &c in &arena[id].children {
            dump(arena, c, indent + 2, out);
        }
    }
    let mut s = String::new();
    dump(&arena, root, 0, &mut s);
    println!("{}", s);
    let lf_count = count_type(&arena, root, SyntaxT::LatexFragment);
    assert!(
        lf_count > 0,
        "expected at least one LatexFragment in table formula context, got 0"
    );
}

#[test]
fn dollar_followed_by_symbol_char_is_not_latex_fragment() {
    let n = get_type_count("$x$+y\n", SyntaxT::LatexFragment, ParseGranularity::Object);
    assert_eq!(
        n, 0,
        "'$x$+y' must not be a LatexFragment ('+' is symbol syntax), got {}",
        n
    );

    let n_ok = get_type_count("$x$ y\n", SyntaxT::LatexFragment, ParseGranularity::Object);
    assert_eq!(
        n_ok, 1,
        "'$x$ y' must be a LatexFragment (space post-char), got {}",
        n_ok
    );
}

#[test]
fn custom_id_is_subscript() {
    let input = "连接到'CUSTOM_ID'属性\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let scripts = find_script(&arena, root);
    assert!(
        !scripts.is_empty(),
        "CUSTOM_ID should contain a Subscript node"
    );
    let script_id = scripts[0];
    let loc = arena[script_id].location;
    assert_eq!(
        &input[loc.start..loc.end],
        "_ID",
        "Subscript should cover '_ID'"
    );
}
