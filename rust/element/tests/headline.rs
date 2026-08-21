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

use org_element::testutils::*;

#[test]
fn basic_headline() {
    for (input, desc) in &[
        ("* \n", "minimal"),
        ("****  \n", "four stars"),
        ("* title\n", "with title"),
        ("* TODO [#A] title\n", "with priority"),
        ("* title :tag1:tag2:\n", "with tags"),
        ("* TODO title\n", "with keyword"),
    ] {
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline ({}), found {}", desc, count);
    }
}

#[test]
fn multiple_headlines() {
    let input = "* Level 1\n** Level 2\n*** Level 3\n";
    let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
    assert_eq!(count, 3, "Expected 3 headlines, found {}", count);
}

#[test]
fn headline_with_tab_after_stars() {
    let input = "*\tOne\n**\tSub\n*\tTwo\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let root_children = &arena[root].children;
    let h1 = root_children.first().expect("Expected first headline");
    let Syntax::Headline(_) = &arena[*h1].data else {
        panic!("Expected Headline for h1, got {:?}", arena[*h1].data);
    };
    let h1_end = arena[*h1].location.end;
    let h2 = root_children
        .get(1)
        .expect("Expected second top-level headline");
    let Syntax::Headline(_) = &arena[*h2].data else {
        panic!("Expected Headline for h2, got {:?}", arena[*h2].data);
    };
    assert!(
        h1_end <= arena[*h2].location.start,
        "First headline end ({}) should not overlap second headline start ({}): \
             headlines with tab should form proper subtree boundaries",
        h1_end,
        arena[*h2].location.start
    );
}

#[test]
fn headline_with_unicode_tags() {
    let input = "* title :café:标签:\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let root_children = &arena[root].children;
    let h = root_children.first().expect("Expected headline");
    let Syntax::Headline(data) = &arena[*h].data else {
        panic!("Expected Headline");
    };
    let tag_strs: Vec<&str> = data.tags.iter().map(|t| t.0).collect();
    assert!(
        tag_strs.contains(&"café"),
        "Tags should include unicode 'café', got {:?}",
        tag_strs
    );
    assert!(
        tag_strs.contains(&"标签"),
        "Tags should include CJK '标签', got {:?}",
        tag_strs
    );
}

#[test]
fn headline_comment_not_in_title() {
    let input = "* COMMENT stuff\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let root_children = &arena[root].children;
    let h = root_children.first().expect("Expected headline");
    let Syntax::Headline(data) = &arena[*h].data else {
        panic!("Expected Headline");
    };
    assert!(
        !data.title.starts_with("COMMENT"),
        "Title should not include COMMENT keyword, got: {:?}",
        data.title
    );
}

/// Headline title text is a secondary string in Emacs org-element and is
/// stored in the `:title` property, never in `org-element-contents`.
/// Therefore, parsed title objects (PlainText, Bold, Link, …) must NOT
/// appear as direct children of the Headline node.
///
/// Headline children should only be Section and sub-Headline nodes.
mod title_objects_not_in_children {
    use super::*;

    fn headline_direct_child_types(input: &str) -> Vec<SyntaxT> {
        let bump = Bump::new();
        let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        // root → headline (first child)
        let hl = *arena[root].children.first().expect("expected a headline");
        assert_eq!(SyntaxT::from(&arena[hl].data), SyntaxT::Headline);
        arena[hl]
            .children
            .iter()
            .map(|&id| SyntaxT::from(&arena[id].data))
            .collect()
    }

    /// Title objects must not appear as direct Headline children regardless
    /// of what markup the title contains.
    #[test]
    fn title_objects_not_direct_children() {
        let inline_types = [
            SyntaxT::PlainText,
            SyntaxT::Bold,
            SyntaxT::Italic,
            SyntaxT::Underline,
            SyntaxT::Code,
            SyntaxT::Verbatim,
            SyntaxT::Link,
            SyntaxT::StrikeThrough,
        ];
        for (input, desc) in [
            ("* foo bar\n", "plain text title"),
            ("* foo *bold* bar\n", "title with markup"),
            ("* See [[https://example.com][text]]\n", "title with link"),
            (
                "* Title with *bold*\nsome body text\n** Sub\n",
                "headline with body and sub",
            ),
        ] {
            let types = headline_direct_child_types(input);
            for t in inline_types {
                assert!(
                    !types.contains(&t),
                    "{:?} must not be a direct child of Headline ({}); got {:?}",
                    t,
                    desc,
                    types
                );
            }
        }
    }
}
