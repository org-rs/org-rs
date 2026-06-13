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
fn granularity_levels() {
    for (input, typ, granularity, desc) in &[
        (
            "* Headline\n\nSome paragraph\n",
            SyntaxT::Headline,
            ParseGranularity::Headline,
            "headline",
        ),
        (
            "* Headline\n\n#+BEGIN_CENTER\nCenter\n#+END_CENTER\n",
            SyntaxT::CenterBlock,
            ParseGranularity::GreaterElement,
            "greater element",
        ),
        (
            "* Headline\n\nParagraph text\n",
            SyntaxT::Paragraph,
            ParseGranularity::Element,
            "element",
        ),
        (
            "Paragraph with *bold* text\n",
            SyntaxT::Bold,
            ParseGranularity::Object,
            "object",
        ),
    ] {
        let count = get_type_count(input, *typ, *granularity);
        assert_eq!(
            count, 1,
            "Expected 1 node at {} granularity, found {}",
            desc, count
        );
    }
}
