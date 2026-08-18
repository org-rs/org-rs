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
//!   `root/headline/section/fixed_width` vs `root/headline/section/keyword` —
//!   `  #+RESULTS:` with leading whitespace is not collected as an affiliated
//!   keyword, so the `: output` line is parsed as a standalone FixedWidth
//!   instead of one whose `:begin` includes the `#+RESULTS:` line.
//!
//! Corpus origin: emacs_china_elisp.org @323688.
use bumpalo::Bump;
use org_element::data::SyntaxT;
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use org_element::testutils::*;

/// `  #+RESULTS:` has leading whitespace before `#+`.
/// Emacs: one FixedWidth element with `:begin` at the `#+RESULTS:` line.
/// Rust:  a Keyword element + a separate FixedWidth element.
#[test]
fn indented_results_keyword_affiliated_to_fixed_width() {
    let input = "  #+RESULTS:\n  : output\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let kw = count_type(&arena, root, SyntaxT::Keyword);
    let fw = count_type(&arena, root, SyntaxT::FixedWidth);
    assert_eq!(
        kw, 0,
        "#+RESULTS: with leading spaces must be an affiliated keyword, not a Keyword element; got {kw} keywords"
    );
    assert_eq!(
        fw, 1,
        "expected exactly 1 FixedWidth element (with #+RESULTS: as affiliated keyword); got {fw}"
    );
}
