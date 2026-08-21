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

//! Tests for block parsing with affiliated keywords.
//!
//! When `#+attr_texinfo:` (or any affiliated keyword) precedes a
//! `#+begin_quote`, the block's content interval must start AFTER the
//! `#+begin_quote` header line, not at it.  Otherwise the content parser
//! sees `#+begin_quote` inside the content and creates a nested
//! QuoteBlock, producing a `quote_block/quote_block` tree instead of
//! the correct `quote_block/paragraph`.

use bumpalo::Bump;
use org_element::data::SyntaxT;
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use org_element::testutils::*;

#[test]
fn quote_block_with_affiliated_has_no_nested_block() {
    let input = "#+attr_texinfo: :tag Important\n\
                 #+begin_quote\n\
                 Hello world\n\
                 #+end_quote\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let qb_count = count_type(&arena, root, SyntaxT::QuoteBlock);
    assert_eq!(
        qb_count, 1,
        "expected exactly one QuoteBlock, got {qb_count}"
    );

    let p_count = count_type(&arena, root, SyntaxT::Paragraph);
    assert_eq!(
        p_count, 1,
        "expected exactly one Paragraph inside QuoteBlock, got {p_count}"
    );
}

#[test]
fn quote_block_with_affiliated_inside_headline() {
    let input = "* H1\n\
                 :PROPERTIES:\n\
                 :UNNUMBERED: notoc\n\
                 :END:\n\
                 \n\
                 #+attr_texinfo: :tag Important\n\
                 #+begin_quote\n\
                 Hello world\n\
                 #+end_quote\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let qb_count = count_type(&arena, root, SyntaxT::QuoteBlock);
    assert_eq!(
        qb_count, 1,
        "expected exactly one QuoteBlock, got {qb_count}"
    );

    let p_count = count_type(&arena, root, SyntaxT::Paragraph);
    assert_eq!(p_count, 1, "expected exactly one Paragraph, got {p_count}");
}
