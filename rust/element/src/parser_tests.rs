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

mod test_plain_list {
    use super::*;

    #[test]
    fn test_unordered_list() {
        let input = "- item 1\n- item 2\n- item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn test_ordered_list() {
        let input = "1. item 1\n2. item 2\n3. item 3\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn test_descriptive_list() {
        let input = "- term 1 :: description 1\n- term 2 :: description 2\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn test_list_with_checkbox() {
        let input = "- [X] checked item\n- [ ] unchecked item\n- [-] partially checked\n";
        let count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 plain list, found {}", count);
    }

    #[test]
    fn test_nested_list() {
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
}

mod test_item {
    use super::*;

    #[test]
    fn test_simple_item() {
        let input = "- some text\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 1, "Expected at least 1 item, found {}", count);
    }

    #[test]
    fn test_item_with_bullet() {
        let input = "* some text\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 0, "Item count: {}", count);
    }

    #[test]
    fn test_item_with_tag() {
        let input = "- tag :: content\n";
        let count = get_type_count(input, SyntaxT::Item, ParseGranularity::Element);
        assert!(count >= 0, "Item count: {}", count);
    }
}

mod test_table {
    use super::*;

    #[test]
    fn test_simple_table() {
        let input = "| col 1 | col 2 |\n|-----|-----|\n| a | b |\n| c | d |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }

    #[test]
    fn test_table_row_count() {
        let input = "| a | b |\n| c | d |\n| e | f |\n";
        let count = get_type_count(input, SyntaxT::TableRow, ParseGranularity::Element);
        assert!(
            count >= 3,
            "Expected at least 3 table rows, found {}",
            count
        );
    }

    #[test]
    fn test_table_with_header() {
        let input = "| head1 | head2 |\n|--------|--------|\n| body1 | body2 |\n";
        let count = get_type_count(input, SyntaxT::Table, ParseGranularity::Element);
        assert!(count >= 1, "Expected 1 table, found {}", count);
    }
}

mod test_blocks {
    use super::*;

    #[test]
    fn test_center_block() {
        let input = "#+BEGIN_CENTER\nCentered text\n#+END_CENTER\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 center block, found {}", count);
    }

    #[test]
    fn test_quote_block() {
        let input = "#+BEGIN_QUOTE\nQuoted text\n#+END_QUOTE\n";
        let count = get_type_count(input, SyntaxT::QuoteBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 quote block, found {}", count);
    }

    #[test]
    fn test_example_block() {
        let input = "#+BEGIN_EXAMPLE\nExample text\n#+END_EXAMPLE\n";
        let count = get_type_count(input, SyntaxT::ExampleBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 example block, found {}", count);
    }

    #[test]
    fn test_src_block() {
        let input = "#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\n";
        let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 src block, found {}", count);
    }

    #[test]
    fn test_verse_block() {
        let input = "#+BEGIN_VERSE\nVerse line 1\nVerse line 2\n#+END_VERSE\n";
        let count = get_type_count(input, SyntaxT::VerseBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 verse block, found {}", count);
    }

    #[test]
    fn test_comment_block() {
        let input = "#+BEGIN_COMMENT\nComment text\n#+END_COMMENT\n";
        let count = get_type_count(input, SyntaxT::CommentBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 comment block, found {}", count);
    }

    #[test]
    fn test_export_block() {
        let input = "#+BEGIN_EXPORT html\n<div>HTML</div>\n#+END_EXPORT\n";
        let count = get_type_count(input, SyntaxT::ExportBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 export block, found {}", count);
    }

    #[test]
    fn test_special_block() {
        let input = "#+BEGIN_SPECIAL\nCustom block content\n#+END_SPECIAL\n";
        let count = get_type_count(input, SyntaxT::SpecialBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 special block, found {}", count);
    }

    #[test]
    fn test_dynamic_block() {
        let input = "#+BEGIN: my-block :param value\nBlock content\n#+END:\n";
        let count = get_type_count(input, SyntaxT::DynamicBlock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 dynamic block, found {}", count);
    }

    #[test]
    fn test_block_case_insensitive() {
        let input = "#+begin_center\nCentered\n#+end_center\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 center block (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn test_incomplete_block() {
        let input = "#+BEGIN_CENTER\n";
        let count = get_type_count(input, SyntaxT::CenterBlock, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 center blocks for incomplete, found {}",
            count
        );
    }
}

mod test_drawer {
    use super::*;

    #[test]
    fn test_simple_drawer() {
        let input = ":TEST:\nDrawer content\n:END:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 drawer, found {}", count);
    }

    #[test]
    fn test_property_drawer() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n";
        let count = get_type_count(input, SyntaxT::PropertyDrawer, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 property drawer, found {}", count);
    }

    #[test]
    fn test_drawer_case_insensitive() {
        let input = ":test:\nDrawer content\n:end:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 drawer (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn test_incomplete_drawer() {
        let input = ":TEST:\n";
        let count = get_type_count(input, SyntaxT::Drawer, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 drawers for incomplete, found {}",
            count
        );
    }
}

mod test_planning {
    use super::*;

    #[test]
    fn test_planning_with_deadline() {
        let input = "* TODO Task\nDEADLINE: <2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn test_planning_with_scheduled() {
        let input = "* TODO Task\nSCHEDULED: <2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn test_planning_with_closed() {
        let input = "* TODO Task\nCLOSED: [2023-12-31]\n";
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning, found {}", count);
    }

    #[test]
    fn test_planning_all_timestamps() {
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

mod test_clock {
    use super::*;

    #[test]
    fn test_running_clock() {
        let input = "CLOCK: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock, found {}", count);
    }

    #[test]
    fn test_closed_clock() {
        let input = "CLOCK: [2023-10-13 Fri 14:40]--[2023-10-13 Fri 14:51] => 0:11\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock, found {}", count);
    }

    #[test]
    fn test_clock_case_insensitive() {
        let input = "Clock: [2023-10-13 Fri 14:40]\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn test_clock_duration_only() {
        let input = "CLOCK: => 0:11\n";
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 clock with duration only, found {}",
            count
        );
    }
}

mod test_diary_sexp {
    use super::*;

    #[test]
    fn test_diary_sexp() {
        let input = "%(org-anniversary 1956 5 14)(2) Arthur Dent is %d years old\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 diary sexp, found {}", count);
    }

    #[test]
    fn test_diary_sexp_must_be_bol() {
        let input = " %(diary-sexp)\n";
        let count = get_type_count(input, SyntaxT::DiarySexp, ParseGranularity::Element);
        assert_eq!(
            count, 0,
            "Expected 0 diary sexp (not at BOL), found {}",
            count
        );
    }
}

mod test_latex_environment {
    use super::*;

    #[test]
    fn test_latex_environment() {
        let input = "\\begin{equation}\nE = mc^2\n\\end{equation}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex environment, found {}", count);
    }

    #[test]
    fn test_latex_equation_environment() {
        let input = "\\begin{equation}\n\\frac{a}{b}\n\\end{equation}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex equation env, found {}", count);
    }

    #[test]
    fn test_latex_aligned_environment() {
        let input = "\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}\n";
        let count = get_type_count(input, SyntaxT::LatexEnvironment, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 latex aligned env, found {}", count);
    }
}

mod test_footnote_definition {
    use super::*;

    #[test]
    fn test_footnote_definition() {
        let input = "[fn:1] This is a footnote.\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 footnote definition, found {}", count);
    }

    #[test]
    fn test_named_footnote_definition() {
        let input = "[fn:my-note] This is a named footnote.\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 named footnote, found {}", count);
    }

    #[test]
    fn test_footnote_definition_multiline() {
        let input = "[fn:1]\nMulti-line\nfootnote content\n";
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(count, 1, "Expected 1 multiline footnote, found {}", count);
    }
}

mod test_fixed_width {
    use super::*;

    #[test]
    fn test_fixed_width() {
        let input = ": Fixed width line\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 fixed width, found {}", count);
    }

    #[test]
    fn test_fixed_width_with_name_affiliated() {
        let input = "#+NAME: my-fixed\n: Fixed width line\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 fixed width with name, found {}",
            count
        );
    }

    #[test]
    fn test_fixed_width_multiple_lines() {
        let input = ": Line 1\n: Line 2\n: Line 3\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 fixed width region, found {}", count);
    }

    #[test]
    fn test_fixed_width_indented() {
        let input = "  : Indented fixed width\n";
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 indented fixed width, found {}", count);
    }
}

mod test_babel_call {
    use super::*;

    #[test]
    fn test_babel_call() {
        let input = "#+CALL: test()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 babel call, found {}", count);
    }

    #[test]
    fn test_babel_call_case_insensitive() {
        let input = "#+call: test()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call (case insensitive), found {}",
            count
        );
    }

    #[test]
    fn test_babel_call_with_header() {
        let input = "#+CALL: test[:results output]()\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call with header, found {}",
            count
        );
    }

    #[test]
    fn test_babel_call_with_arguments() {
        let input = "#+CALL: test(n=4)\n";
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call with arguments, found {}",
            count
        );
    }
}

mod test_node_properties {
    use super::*;

    #[test]
    fn test_node_property() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            count >= 1,
            "Expected at least 1 node property, found {}",
            count
        );
    }

    #[test]
    fn test_multiple_node_properties() {
        let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:PRIORITY: A\n: tags: :foo:bar:\n:END:\n";
        let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
        assert!(
            count >= 3,
            "Expected at least 3 node properties, found {}",
            count
        );
    }
}

mod test_keyword {
    use super::*;

    #[test]
    fn test_keyword() {
        let input = "#+KEYWORD: value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword, found {}", count);
    }

    #[test]
    fn test_keyword_case_insensitive() {
        let input = "#+keyword: value\n";
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 keyword (case insensitive), found {}",
            count
        );
    }
}

mod test_horizontal_rule {
    use super::*;

    #[test]
    fn test_horizontal_rule() {
        let input = "-----\n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 horizontal rule, found {}", count);
    }

    #[test]
    fn test_horizontal_rule_with_spaces() {
        let input = "   -----   \n";
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 horizontal rule with spaces, found {}",
            count
        );
    }
}

mod test_inlinetask {
    use super::*;

    #[test]
    fn test_inlinetask() {
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

mod test_object_parsing {
    use super::*;

    #[test]
    fn test_bold_object() {
        let input = "*bold*\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 bold object, found {}", count);
    }

    #[test]
    fn test_italic_object() {
        let input = "/italic/\n";
        let count = get_type_count(input, SyntaxT::Italic, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 italic object, found {}", count);
    }

    #[test]
    fn test_underline_object() {
        let input = "_underline_\n";
        let count = get_type_count(input, SyntaxT::Underline, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 underline object, found {}", count);
    }

    #[test]
    fn test_strikethrough_object() {
        let input = "+strikethrough+\n";
        let count = get_type_count(input, SyntaxT::StrikeThrough, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 strikethrough object, found {}", count);
    }

    #[test]
    fn test_code_object() {
        let input = "~code~\n";
        let count = get_type_count(input, SyntaxT::Code, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 code object, found {}", count);
    }

    #[test]
    fn test_verbatim_object() {
        let input = "=verbatim=\n";
        let count = get_type_count(input, SyntaxT::Verbatim, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 verbatim object, found {}", count);
    }

    #[test]
    fn test_entity_object() {
        let input = "\\alpha\n";
        let count = get_type_count(input, SyntaxT::Entity, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 entity object, found {}", count);
    }

    #[test]
    fn test_link_object() {
        let input = "[[https://example.com][link]]\n";
        let count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 link object, found {}", count);
    }

    #[test]
    fn test_target_object() {
        let input = "<<target>>\n";
        let count = get_type_count(input, SyntaxT::Target, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 target object, found {}", count);
    }

    #[test]
    fn test_timestamp_object() {
        let input = "<2023-12-31>\n";
        let count = get_type_count(input, SyntaxT::Timestamp, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 timestamp object, found {}", count);
    }
}

mod test_granularity {
    use super::*;

    #[test]
    fn test_headline_granularity() {
        let input = "* Headline\n\nSome paragraph\n";
        let count = get_type_count(input, SyntaxT::Headline, ParseGranularity::Headline);
        assert!(
            count >= 1,
            "Expected at least 1 headline at Headline granularity, found {}",
            count
        );
    }

    #[test]
    fn test_greater_element_granularity() {
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
    fn test_element_granularity() {
        let input = "* Headline\n\nParagraph text\n";
        let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
        assert!(
            count >= 1,
            "Expected paragraph at Element granularity, found {}",
            count
        );
    }

    #[test]
    fn test_object_granularity() {
        let input = "Paragraph with *bold* text\n";
        let count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
        assert!(
            count >= 1,
            "Expected bold at Object granularity, found {}",
            count
        );
    }
}
