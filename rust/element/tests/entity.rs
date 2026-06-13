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
fn le_is_entity_not_latex_fragment() {
    let count = get_type_count(r"\le100Mb", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(count, 1, "\\le should parse as Entity, not LatexFragment");
    let count_lf = get_type_count(
        r"\le100Mb",
        SyntaxT::LatexFragment,
        ParseGranularity::Object,
    );
    assert_eq!(count_lf, 0, "\\le should not produce a LatexFragment");
}

#[test]
fn s_is_entity_not_latex_fragment() {
    let count = get_type_count(r"\S100Mb", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(count, 1, "\\S should parse as Entity, not LatexFragment");
    let count_lf = get_type_count(r"\S100Mb", SyntaxT::LatexFragment, ParseGranularity::Object);
    assert_eq!(count_lf, 0, "\\S should not produce a LatexFragment");
}

/// `\frac12` must be parsed as an Entity, not a LatexFragment.
///
/// The same pattern applies to all digit-terminated entity names
/// (`\frac14`, `\frac34`, `\there4`).
#[test]
fn frac12_is_entity_not_latex_fragment() {
    let count = get_type_count(r"\frac12", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "\\frac12 should parse as Entity, not LatexFragment"
    );
    let count_lf = get_type_count(r"\frac12", SyntaxT::LatexFragment, ParseGranularity::Object);
    assert_eq!(count_lf, 0, "\\frac12 should not produce a LatexFragment");
}

#[test]
fn frac34_is_entity_not_latex_fragment() {
    let count = get_type_count(r"\frac34", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "\\frac34 should parse as Entity, not LatexFragment"
    );
    let count_lf = get_type_count(r"\frac34", SyntaxT::LatexFragment, ParseGranularity::Object);
    assert_eq!(count_lf, 0, "\\frac34 should not produce a LatexFragment");
}

#[test]
fn there4_is_entity_not_latex_fragment() {
    let count = get_type_count(r"\there4", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "\\there4 should parse as Entity, not LatexFragment"
    );
    let count_lf = get_type_count(r"\there4", SyntaxT::LatexFragment, ParseGranularity::Object);
    assert_eq!(count_lf, 0, "\\there4 should not produce a LatexFragment");
}

/// `\Office16` contains digits but is NOT a known entity.  It should
/// parse as a LatexFragment (`\Office`) followed by PlainText (`16`),
/// not as an Entity.
#[test]
fn office16_is_latex_fragment_not_entity() {
    let count = get_type_count(
        r"\Office16",
        SyntaxT::LatexFragment,
        ParseGranularity::Object,
    );
    assert_eq!(count, 1, "\\Office16 should parse as LatexFragment");
    let count_ent = get_type_count(r"\Office16", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(count_ent, 0, "\\Office16 should not parse as Entity");
}

#[test]
fn vert_with_empty_braces_is_entity() {
    let count = get_type_count(r"\vert{}def", SyntaxT::Entity, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "\\vert{{}} should parse as Entity, not LatexFragment"
    );
    let count_lf = get_type_count(
        r"\vert{}def",
        SyntaxT::LatexFragment,
        ParseGranularity::Object,
    );
    assert_eq!(count_lf, 0, "\\vert{{}} should not produce a LatexFragment");
}
