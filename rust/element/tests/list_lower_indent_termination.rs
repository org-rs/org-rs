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
//!   `root/headline/section/plain_list` — missing in Rust when a lower-indented
//!   bullet terminates the outer list.
//!
//! When a list item has a smaller string-width indent than the list's first
//! item, Emacs' `org-list-struct` terminates the current list.  Rust currently
//! keeps scanning, folding the lower-indent item into the same structure.
use bumpalo::Bump;
use org_element::data::SyntaxT;
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use org_element::testutils::*;

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
