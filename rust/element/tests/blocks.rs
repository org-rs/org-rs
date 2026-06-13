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
fn block_types() {
    for (input, typ, desc) in &[
        (
            "#+BEGIN_CENTER\nCentered text\n#+END_CENTER\n",
            SyntaxT::CenterBlock,
            "center",
        ),
        (
            "#+BEGIN_VERSE\nVerse line 1\nVerse line 2\n#+END_VERSE\n",
            SyntaxT::VerseBlock,
            "verse",
        ),
        (
            "#+BEGIN_SPECIAL\nCustom block content\n#+END_SPECIAL\n",
            SyntaxT::SpecialBlock,
            "special",
        ),
        (
            "#+BEGIN: my-block :param value\nBlock content\n#+END:\n",
            SyntaxT::DynamicBlock,
            "dynamic",
        ),
        (
            "#+begin_center\nCentered\n#+end_center\n",
            SyntaxT::CenterBlock,
            "center lowercase",
        ),
        (
            "#+BEGIN_CENTER\ncenter\n#+end_center \n",
            SyntaxT::CenterBlock,
            "center trailing space on END",
        ),
    ] {
        let count = get_type_count(input, *typ, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 {} block, found {}", desc, count);
    }
}

#[test]
fn incomplete_block() {
    let input = "#+BEGIN_CENTER\n";
    let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "Expected 0 center blocks for incomplete, found {}",
        count
    );
}

#[test]
fn export_block_type() {
    let input = "#+BEGIN_EXPORT html\n<div>HTML</div>\n#+END_EXPORT\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let block = section_children.first().expect("Expected export block");

    let Syntax::ExportBlock(data) = &arena[*block].data else {
        panic!("Expected ExportBlock, got: {:?}", arena[*block].data);
    };
    assert_eq!(
        data.type_s, "html",
        "Expected export type_s 'html', got {:?}",
        data.type_s
    );
}

#[test]
fn special_block_with_affiliated_keyword() {
    let input = "#+attr_texinfo: :options org-entry-get pom property\n\
                  #+begin_defun\n\
                  Get value of PROPERTY for entry.\n\
                  #+end_defun\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let block = section_children.first().expect("Expected special block");

    let Syntax::SpecialBlock(data) = &arena[*block].data else {
        panic!("Expected SpecialBlock, got: {:?}", arena[*block].data);
    };
    assert_eq!(
        data.type_s, "defun",
        "Expected type_s 'defun', got {:?}",
        data.type_s
    );
}

#[test]
fn src_block_language() {
    let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let block = section_children.first().expect("Expected src block");

    let Syntax::SrcBlock(data) = &arena[*block].data else {
        panic!("Expected SrcBlock, got: {:?}", arena[*block].data);
    };
    assert_eq!(
        data.language,
        Some("python"),
        "Expected language 'python', got {:?}",
        data.language
    );
}
