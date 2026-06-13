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

//! Tests for macro object parsing (`{{{name(args)}}}`).
//!
//! Discovered via the corpus check against `corpus/orgmode_guide.org`
//! (the official Org "Compact Guide", GFDL 1.3), which contains 76
//! `{{{kbd(...)}}}` / `{{{version}}}` macros.  Emacs `org-element`
//! recognises each as a `macro` object, but org-rs currently emits no
//! `Macro` node at all, leaving the `{{{...}}}` text as plain text.
//!
//! Corpus discrepancy class:
//!   `root/headline/section/paragraph/macro` (missing-in-rust)
//!
//! Emacs reference (`{{{kbd(C-c C-c)}}}`):
//!   key="kbd" args=("C-c C-c") begin=7 value="{{{kbd(C-c C-c)}}}"

mod common;
use bumpalo::Bump;
use org_element::data::{NodeArena, NodeId, Syntax};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

fn find_macros(arena: &NodeArena, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    if matches!(arena[id].data, Syntax::Macro(_)) {
        out.push(id);
    }
    for &child in &arena[id].children {
        out.extend(find_macros(arena, child));
    }
    out
}

#[test]
fn macro_with_args_is_detected() {
    let input = "Press {{{kbd(C-c C-c)}}} now.\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let macros = find_macros(&arena, root);
    assert_eq!(
        macros.len(),
        1,
        "expected exactly one Macro node for `{{{{{{kbd(C-c C-c)}}}}}}`, got {}",
        macros.len()
    );

    let id = macros[0];
    let loc = arena[id].location;
    // 0-based byte offset of the first `{` (Emacs reports 1-based begin=7).
    assert_eq!(loc.start, 6, "Macro should begin at the first `{{`");
    assert_eq!(
        &input[loc.start..loc.end],
        "{{{kbd(C-c C-c)}}} ",
        "Macro span should cover `{{{{{{...}}}}}}` plus trailing whitespace (Emacs :end includes post-blank)"
    );

    if let Syntax::Macro(data) = &arena[id].data {
        assert_eq!(data.key, "kbd", "macro key");
        assert_eq!(data.args, vec!["C-c C-c"], "macro args");
    } else {
        unreachable!("node is not a Macro");
    }
}

#[test]
fn macro_without_args_is_detected() {
    // `{{{version}}}` appears in the guide's `#+subtitle:` and body text.
    let input = "Release {{{version}}}\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let macros = find_macros(&arena, root);
    assert_eq!(
        macros.len(),
        1,
        "expected one Macro node for `{{{{{{version}}}}}}`, got {}",
        macros.len()
    );

    if let Syntax::Macro(data) = &arena[macros[0]].data {
        assert_eq!(data.key, "version", "macro key");
        assert!(data.args.is_empty(), "argless macro should have no args");
    } else {
        unreachable!("node is not a Macro");
    }
}

#[test]
fn macro_with_args_spanning_newline() {
    // `{{{kbd(C-c\nC-c)}}}` appears in the guide's footnote text.
    let input = "before {{{kbd(C-c\nC-c)}}} after\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let macros = find_macros(&arena, root);
    assert_eq!(
        macros.len(),
        1,
        "expected one Macro node for multiline `{{{{{{kbd(...)}}}}}}`, got {}",
        macros.len()
    );

    if let Syntax::Macro(data) = &arena[macros[0]].data {
        assert_eq!(data.key, "kbd", "macro key");
        assert_eq!(data.args, vec!["C-c\nC-c"], "macro args preserve newline");
    } else {
        unreachable!("node is not a Macro");
    }
}
