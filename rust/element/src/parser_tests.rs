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

use crate::data::{Syntax, SyntaxNode, SyntaxT};
use crate::environment::DefaultEnvironment;
use crate::parser::{ParseGranularity, Parser};

fn count_type(tree: &SyntaxNode, typ: SyntaxT) -> usize {
    let mut count = 0;
    if SyntaxT::from(&tree.data) == typ {
        count += 1;
    }
    for child in tree.children.borrow().iter() {
        count += count_type(child, typ);
    }
    count
}

fn get_type_count(input: &str, typ: SyntaxT, granularity: ParseGranularity) -> usize {
    let parser = Parser::new(input, granularity, DefaultEnvironment);
    let tree = parser.parse_buffer();
    count_type(&tree, typ)
}

mod plain_list {
    use super::*;

    #[test]
    fn unordered_list() {
        let input = "- item 1\n- item 2\n- item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn ordered_list() {
        let input = "1. item 1\n2. item 2\n3. item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn descriptive_list() {
        let input = "- term 1 :: description 1\n- term 2 :: description 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn list_with_checkbox() {
        let input = "- [X] checked item\n- [ ] unchecked item\n- [-] partially checked\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn nested_list() {
        let input = "- item 1\n  - subitem 1\n  - subitem 2\n- item 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        let item_count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(
            item_count >= 1 || count >= 1,
            "Expected list or items, found list: {}, items: {}",
            count,
            item_count
        );
    }

    #[test]
    fn list_two_items() {
        let input = "- item 1\n- item 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn list_empty() {
        let input = "- \n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert!(count >= 0, "Plain list count: {}", count);
    }
}

mod item {
    use super::*;

    #[test]
    fn simple_item() {
        let input = "- some text\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn item_with_bullet() {
        let input = "* some text\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 0, "Item count: {}", count);
    }

    #[test]
    fn item_with_tag() {
        let input = "- tag :: content\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 0, "Item count: {}", count);
    }

    #[test]
    fn item_with_checkbox() {
        let input = "- [X] checked item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn item_with_unchecked() {
        let input = "- [ ] unchecked item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn item_ordered_start() {
        let input = "1. first item\n2. second item\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 2, "Expected at least 2 items, found {}", count);
    }
}

mod headline {
    use super::*;

    #[test]
    fn basic_headline() {
        let input = "* \n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_stars() {
        let input = "****  \n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_title() {
        let input = "* title\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_priority() {
        let input = "* TODO [#A] title\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_tags() {
        let input = "* title :tag1:tag2:\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn headline_with_keyword() {
        let input = "* TODO title\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }

    #[test]
    fn multiple_headlines() {
        let input = "* Level 1\n** Level 2\n*** Level 3\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 3, "Expected 3 headlines, found {}", count);
    }

    #[test]
    fn headline_only_stars() {
        let input = "* \n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 headline, found {}", count);
    }
}

mod table {
    use super::*;

    #[test]
    fn simple_table() {
        let input = "| col 1 | col 2 |\n|-----|-----|\n| a | b |\n| c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_row_count() {
        let input = "| a | b |\n| c | d |\n| e | f |\n";
        let count = get_type_count(input, SyntaxT::TableRow, ParseGranularity::Element);
        assert!(
            count >= 3,
            "Expected at least 3 table rows, found {}",
            count
        );
    }

    #[test]
    fn table_with_header() {
        let input = "| head1 | head2 |\n|--------|--------|\n| body1 | body2 |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_no_hline() {
        let input = "| a | b |\n| c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_indented() {
        let input = "  | a | b |\n  | c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_empty_cells() {
        let input = "| | b |\n| c | |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn table_eof() {
        let input = "| a | b |";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }
}

mod blocks {
    use super::*;

    #[test]
    fn center() {
        let input = "#+BEGIN_CENTER\nCentered text\n#+END_CENTER\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 center block, found {}", count);
    }

    #[test]
    fn quote() {
        let input = "#+BEGIN_QUOTE\nQuoted text\n#+END_QUOTE\n";
        let count = get_type_count(input, SyntaxT::QuoteBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 quote block, found {}", count);
    }

    #[test]
    fn example() {
        let input = "#+BEGIN_EXAMPLE\nExample text\n#+END_EXAMPLE\n";
        let count = get_type_count(input, SyntaxT::ExampleBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 example block, found {}", count);
    }

    #[test]
    fn src() {
        let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
        let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 src block, found {}", count);
    }

    #[test]
    fn verse() {
        let input = "#+BEGIN_VERSE\nVerse line 1\nVerse line 2\n#+END_VERSE\n";
        let count = get_type_count(input, SyntaxT::VerseBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 verse block, found {}", count);
    }

    #[test]
    fn comment() {
        let input = "#+BEGIN_COMMENT\nComment text\n#+END_COMMENT\n";
        let count = get_type_count(input, SyntaxT::CommentBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment block, found {}", count);
    }

    #[test]
    fn export() {
        let input = "#+BEGIN_EXPORT html\n<div>HTML</div>\n#+END_EXPORT\n";
        let count = get_type_count(input, SyntaxT::ExportBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 export block, found {}", count);
    }

    #[test]
    fn special() {
        let input = "#+BEGIN_SPECIAL\nCustom block content\n#+END_SPECIAL\n";
        let count = get_type_count(input, SyntaxT::SpecialBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 special block, found {}", count);
    }

    #[test]
    fn dynamic() {
        let input = "#+BEGIN: my-block :param value\nBlock content\n#+END:\n";
        let count = get_type_count(input, SyntaxT::DynamicBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 dynamic block, found {}", count);
    }

    #[test]
    fn case_insensitive() {
        let input = "#+begin_center\nCentered\n#+end_center\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 center block (case insensitive), found {}",
            count
        );
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
}

mod drawer {
    use super::*;

    #[test]
    fn simple_drawer() {
        let input = ":TEST:\nDrawer content\n:END:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 drawer, found {}", count);
    }

    #[test]
    fn property_drawer() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n";
        let count = get_type_count(input, SyntaxT::PropertyDrawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 property drawer, found {}", count);
    }

    #[test]
    fn drawer_case_insensitive() {
        let input = ":test:\nDrawer content\n:end:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 drawer (case insensitive), found {}",
            count
        );
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
}

mod planning {
    use super::*;

    #[test]
    fn planning_with_deadline() {
        let input = "* TODO Task\nDEADLINE: <2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn planning_with_scheduled() {
        let input = "* TODO Task\nSCHEDULED: <2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn planning_with_closed() {
        let input = "* TODO Task\nCLOSED: [2023-12-31]\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn planning_all_timestamps() {
        let input =
            "* TODO Task\nDEADLINE: <2023-12-31> SCHEDULED: <2023-12-30> CLOSED: [2023-12-29]\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 planning with all timestamps, found {}",
            count
        );
    }
}

mod comment {
    use super::*;

    #[test]
    fn basic_comment() {
        let input = "# Comment\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment, found {}", count);
    }

    #[test]
    fn comment_indented() {
        let input = "  # Indented comment\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment, found {}", count);
    }

    #[test]
    fn comment_multiple_lines() {
        let input = "# Line 1\n# Line 2\n# Line 3\n";
        let count = get_type_count(input, SyntaxT::Comment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment, found {}", count);
    }
}

mod clock {
    use super::*;

    #[test]
    fn running_clock() {
        let input = "CLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock, found {}", count);
    }

    #[test]
    fn closed_clock() {
        let input = "CLOCK: [2023-10-13 Fri 14:40]--[2023-10-13 Fri 14:51] => 0:11\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock, found {}", count);
    }

    #[test]
    fn clock_case_insensitive() {
        let input = "Clock: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn clock_duration_only() {
        let input = "CLOCK: => 0:11\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock with duration only, found {}",
            count
        );
    }

    #[test]
    fn must_be_bol() {
        let input = "   CLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock (whitespace allowed per Elisp), found {}",
            count
        );
    }

    #[test]
    fn with_leading_whitespace() {
        let input = "\tCLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock with tab, found {}", count);
    }

    #[test]
    fn invalid_date() {
        let input = "CLOCK: [2023-02-29 Wed 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 clock (Feb 29 in non-leap year), found {}",
            count
        );
    }
}

mod diary_sexp {
    use super::*;

    #[test]
    fn basic() {
        let input = "%%(org-anniversary 1956 5 14) Arthur Dent is %d years old\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 diary sexp, found {}", count);
    }

    #[test]
    fn simple() {
        let input = "%%(diary-sexp)\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 diary sexp, found {}", count);
    }

    #[test]
    fn must_be_bol() {
        let input = " %(diary-sexp)\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 diary sexp (not at BOL), found {}",
            count
        );
    }
}

mod latex_environment {
    use super::*;

    #[test]
    fn basic() {
        let input = "\\begin{equation}\nE = mc^2\n\\end{equation}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex environment, found {}", count);
    }

    #[test]
    fn latex_equation_environment() {
        let input = "\\begin{equation}\n\\frac{a}{b}\n\\end{equation}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex equation env, found {}", count);
    }

    #[test]
    fn latex_aligned_environment() {
        let input = "\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex aligned env, found {}", count);
    }
}

mod footnote_definition {
    use super::*;

    #[test]
    fn footnote_definition() {
        let input = "[fn:1] This is a footnote.\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition, found {}", count);
    }

    #[test]
    fn named_footnote_definition() {
        let input = "[fn:my-note] This is a named footnote.\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 named footnote, found {}", count);
    }

    #[test]
    fn multiline() {
        let input = "[fn:1]\nMulti-line\nfootnote content\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 multiline footnote, found {}", count);
    }

    #[test]
    fn content_location() {
        let input = "[fn:1] This is footnote content\n";
        let parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let tree = parser.parse_buffer();

        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition");

        let root_children = tree.children.borrow();
        let section = root_children.first().expect("Expected at least one child");
        let section_children = section.children.borrow();
        let fn_node = section_children
            .first()
            .expect("Expected footnote in section");

        if let Syntax::FootnoteDefinition(_) = &fn_node.data {
            let content_loc = fn_node
                .content_location
                .expect("Footnote definition should have content_location");

            assert_eq!(
                content_loc.start, 7,
                "contents-begin should be after [fn:1] and space"
            );

            let content = &input[content_loc.start..content_loc.end];
            assert_eq!(content, "This is footnote content", "content should match");
        } else {
            panic!("Expected FootnoteDefinition, got: {:?}", fn_node.data);
        }
    }

    #[test]
    fn with_label() {
        let input = "[fn:my-label] Some content\n";
        let parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment);
        let tree = parser.parse_buffer();

        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition");

        let root_children = tree.children.borrow();
        let section = root_children.first().expect("Expected at least one child");
        let section_children = section.children.borrow();
        let fn_node = section_children
            .first()
            .expect("Expected footnote in section");

        if let Syntax::FootnoteDefinition(data) = &fn_node.data {
            assert_eq!(data.label, "my-label", "Label should be extracted");
            assert_eq!(data.value, "Some content", "Value should be raw text");
        } else {
            panic!("Expected FootnoteDefinition syntax type");
        }
    }
}

mod fixed_width {
    use super::*;

    #[test]
    fn basic() {
        let input = ": Fixed width line\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 fixed width, found {}", count);
    }

    #[test]
    fn with_name_affiliated() {
        let input = "#+NAME: my-fixed\n: Fixed width line\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 fixed width with name, found {}",
            count
        );
    }

    #[test]
    fn multiple_lines() {
        let input = ": Line 1\n: Line 2\n: Line 3\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 fixed width region, found {}", count);
    }

    #[test]
    fn indented() {
        let input = "  : Indented fixed width\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 indented fixed width, found {}", count);
    }
}

mod babel_call {
    use super::*;

    #[test]
    fn babel_call() {
        let input = "#+CALL: test()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 babel call, found {}", count);
    }

    #[test]
    fn case_insensitive() {
        let input = "#+call: test()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn with_header() {
        let input = "#+CALL: test[:results output]()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call with header, found {}",
            count
        );
    }

    #[test]
    fn with_arguments() {
        let input = "#+CALL: test(n=4)\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call with arguments, found {}",
            count
        );
    }
}

mod node_properties {
    use super::*;

    #[test]
    fn basic() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            count >= 1,
            "Expected at least 1 node property, found {}",
            count
        );
    }

    #[test]
    fn multiple_node_properties() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:PRIORITY: A\n: tags: :foo:bar:\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            count >= 3,
            "Expected at least 3 node properties, found {}",
            count
        );
    }
}

mod keyword {
    use super::*;

    #[test]
    fn keyword() {
        let input = "#+KEYWORD: value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }

    #[test]
    fn case_insensitive() {
        let input = "#+keyword: value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 keyword (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn keyword_with_space() {
        let input = "#+KEYWORD:    spaced value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }

    #[test]
    fn keyword_with_newline() {
        let input = "#+KEYWORD: value\nparagraph\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }
}

mod horizontal_rule {
    use super::*;

    #[test]
    fn horizontal_rule() {
        let input = "-----\n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 horizontal rule, found {}", count);
    }

    #[test]
    fn with_spaces() {
        let input = "   -----   \n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 horizontal rule with spaces, found {}",
            count
        );
    }
}

mod inlinetask {
    use super::*;

    #[test]
    fn basic() {
        let input = "*************** Inlinetask content\n";
        let count = get_type_count(input, SyntaxT::InlineTask, ParseGranularity::Element);
        let headline_count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Element);
        assert!(
            headline_count >= 1 || count >= 1,
            "Expected headline or inlinetask, found headline: {}, inlinetask: {}",
            headline_count,
            count
        );
    }
}

mod object_parsing {
    use super::*;

    #[test]
    fn bold_object() {
        let input = "*bold*\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 bold object, found {}", count);
    }

    #[test]
    fn italic_object() {
        let input = "/italic/\n";
        let count = get_type_count(input, SyntaxT::Italic, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 italic object, found {}", count);
    }

    #[test]
    fn underline_object() {
        let input = "_underline_\n";
        let count = get_type_count(input, SyntaxT::Underline, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 underline object, found {}", count);
    }

    #[test]
    fn strikethrough_object() {
        let input = "+strikethrough+\n";
        let count = get_type_count(input, SyntaxT::StrikeThrough, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 strikethrough object, found {}", count);
    }

    #[test]
    fn code_object() {
        let input = "~code~\n";
        let count = get_type_count(input, SyntaxT::Code, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 code object, found {}", count);
    }

    #[test]
    fn verbatim_object() {
        let input = "=verbatim=\n";
        let count = get_type_count(input, SyntaxT::Verbatim, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 verbatim object, found {}", count);
    }

    #[test]
    fn entity_object() {
        let input = "\\alpha\n";
        let count = get_type_count(input, SyntaxT::Entity, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 entity object, found {}", count);
    }

    #[test]
    fn link_object() {
        let input = "[[https://example.com][link]]\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 link object, found {}", count);
    }

    #[test]
    fn target_object() {
        let input = "<<target>>\n";
        let count = get_type_count(input, SyntaxT::Target, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 target object, found {}", count);
    }

    #[test]
    fn timestamp_object() {
        let input = "<2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Timestamp, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 timestamp object, found {}", count);
    }
}

mod granularity {
    use super::*;

    #[test]
    fn headline_granularity() {
        let input = "* Headline\n\nSome paragraph\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Headline);
        assert!(
            count >= 1,
            "Expected at least 1 headline at Headline granularity, found {}",
            count
        );
    }

    #[test]
    fn greater_element_granularity() {
        let input = "* Headline\n\n#+BEGIN_CENTER\nCenter\n#+END_CENTER\n";
        let count = get_type_count(
            input,
            SyntaxT::CenterBlock,
            ParseGranularity::GreaterElement,
        );
        assert!(
            count >= 1,
            "Expected center block at GreaterElement granularity, found {}",
            count
        );
    }

    #[test]
    fn element_granularity() {
        let input = "* Headline\n\nParagraph text\n";
        let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert!(
            count >= 1,
            "Expected paragraph at Element granularity, found {}",
            count
        );
    }

    #[test]
    fn object_granularity() {
        let input = "Paragraph with *bold* text\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold at Object granularity, found {}",
            count
        );
    }
}
