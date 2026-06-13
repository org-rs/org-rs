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
//!
//! Corpus discrepancy class:
//!   `root/headline/section/drawer/paragraph` — extra paragraph at the drawer
//!   header line when `#+RESULTS:` is an affiliated keyword.
//!
//! When `drawer_content_bounds` uses `span.start` (the affiliated keyword
//! position) instead of `content_start` (the `:DRAWERNAM:` position), the
//! content region begins at the drawer header line.  The parser then tries
//! to parse `:RESULTS:` as a nested element, fails, and creates a fallback
//! paragraph there.
//!
//! Corpus origin: emacs_china_elisp.org @125432.
mod common;
use bumpalo::Bump;
use common::*;
use org_element::data::{NodeArena, NodeId, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

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

    assert_eq!(
        drawers.len(),
        1,
        "expected exactly 1 Drawer; got {}",
        drawers.len()
    );
    let drawer_id = drawers[0];
    let para_count = count_type(&arena, drawer_id, SyntaxT::Paragraph);
    assert_eq!(
        para_count, 1,
        "Drawer must have exactly 1 Paragraph child (the content, not the header line); got {para_count}"
    );
}
