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
fn drawers_simple_and_case_insensitive() {
    for (input, desc) in &[
        (":TEST:\nDrawer content\n:END:\n", "basic"),
        (":test:\nDrawer content\n:end:\n", "case insensitive"),
    ] {
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 drawer ({}), found {}", desc, count);
    }
}

#[test]
fn property_drawer_and_node_properties() {
    for (input, expected_props, desc) in &[
        (
            ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n",
            1_usize,
            "single property",
        ),
        (
            ":PROPERTIES:\n:ID: test-id\n:CUSTOM_ID: custom-id\n:END:\n",
            2,
            "two properties",
        ),
    ] {
        let pd_count = get_type_count(input, SyntaxT::PropertyDrawer, ParseGranularity::Element);
        assert_eq!(
            pd_count, 1,
            "Expected 1 property drawer ({}), found {}",
            desc, pd_count
        );
        let np_count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert_eq!(
            np_count, *expected_props,
            "Expected {} node properties ({}), found {}",
            expected_props, desc, np_count
        );
    }
}

#[test]
fn incomplete_drawer() {
    let input = ":TEST:\n";
    let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "Expected 0 drawers for incomplete, found {}",
        count
    );
}

/// Emacs parses `:PROPERTIES:` drawers that appear right after a
/// headline body (before the blank line that ends the section's
/// first paragraph). Rust absorbs the drawer into a Paragraph
/// instead of creating a PropertyDrawer node.
#[test]
fn property_drawer_after_headline_no_blank_line() {
    let input = "** my-comment-box\n\
                      :PROPERTIES:\n\
                      :CREATED:  [2019-01-11 Fri 14:36]\n\
                      :END:\n\n\
                      paragraph after blank line\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let pd_count = count_type(&arena, root, SyntaxT::PropertyDrawer);
    assert!(
        pd_count > 0,
        "Rust absorbs ':' lines after headline body into Paragraph instead of PropertyDrawer"
    );
}

/// A regular drawer placed immediately after a property drawer must
/// have its body parsed into a Paragraph. Regression from
/// `karl_voit_config.org` (`:LINKS:` drawer): Rust recognises the
/// drawer but produces no Paragraph child for its contents.
#[test]
fn paragraph_inside_drawer() {
    let input = "* H\n\
                     :PROPERTIES:\n\
                     :ID:       x\n\
                     :END:\n\
                     :LINKS:\n\
                     [2026-02-16 Mon 19:10] <- [[id:foo][General]]\n\
                     :END:\n\n\
                     text\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    fn drawer_has_para(arena: &NodeArena, id: NodeId, in_drawer: bool) -> bool {
        if in_drawer && matches!(arena[id].data, Syntax::Paragraph) {
            return true;
        }
        let is_drawer = matches!(arena[id].data, Syntax::Drawer(_));
        arena[id]
            .children
            .iter()
            .any(|&c| drawer_has_para(arena, c, in_drawer || is_drawer))
    }
    assert!(
        drawer_has_para(&arena, root, false),
        "Expected a Paragraph inside the :LINKS: drawer; Rust produced none"
    );
}
