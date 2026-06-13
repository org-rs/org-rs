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
//! Corpus discrepancy class:
//!   `root/.../footnote_definition/paragraph/plain_text` — missing in Rust.
//!
//! Emacs' `:contents-end` for a footnote definition includes the final
//! newline of the last content line.  `footnote_definition_parser` trimmed
//! that newline off the content interval, so when the paragraph ended with an
//! object (e.g. a link) there was no room for the trailing `PlainText`
//! newline node that org-element emits.  With single-plain-text content the
//! difference was harmless (the newline merely folded into the lone text
//! node), but a trailing object surfaced it as a missing object — and, in the
//! oracle comparison, made the *following* footnote's content look absent.
//!
//! Corpus origin: emacs_china_org_manual.org @297006 (footnote `[fn:65]`,
//! preceded by `[fn:64]` whose paragraph ends with a `[[...][...]]` link).
mod common;
use bumpalo::Bump;
use common::*;
use org_element::data::{Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

/// A footnote definition whose paragraph ends with a link must keep the
/// trailing newline as a `PlainText` object, exactly as Emacs does.
#[test]
fn footnote_paragraph_ending_in_link_keeps_trailing_newline() {
    let input = "[fn:1] text [[target][label]]\n";

    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    assert_eq!(
        count_type(&arena, root, SyntaxT::Link),
        1,
        "the link must be parsed"
    );

    // Locate the footnote's paragraph and inspect its objects: the last child
    // must be a PlainText holding the trailing newline (Emacs behaviour).
    let section = arena[root].children[0];
    let fn_node = arena[section].children[0];
    let Syntax::FootnoteDefinition(_) = &arena[fn_node].data else {
        panic!("expected FootnoteDefinition, got {:?}", arena[fn_node].data);
    };
    let para = arena[fn_node].children[0];
    let objects = &arena[para].children;

    let last = *objects.last().expect("paragraph must have objects");
    let Syntax::PlainText(text) = &arena[last].data else {
        panic!(
            "last paragraph object must be PlainText (the trailing newline), \
             got {:?}",
            arena[last].data
        );
    };
    assert_eq!(
        *text, "\n",
        "trailing PlainText after the link must be the newline"
    );
}

/// Two footnotes where the first ends with a link: both must be siblings and
/// the second's textual content must be present (it was dropped relative to
/// the oracle before the fix).
#[test]
fn footnote_after_link_terminated_footnote_keeps_content() {
    let input = "[fn:1] text [[target][label]]\n\n[fn:2] more text\n";

    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let section = arena[root].children[0];
    let section_children = &arena[section].children;
    assert_eq!(
        section_children.len(),
        2,
        "the two footnote definitions must be siblings, got {}",
        section_children.len()
    );

    // The second footnote's paragraph must still carry its plain text.
    let fn2 = section_children[1];
    let Syntax::FootnoteDefinition(data) = &arena[fn2].data else {
        panic!("second child must be FootnoteDefinition");
    };
    assert_eq!(data.label, "2");
    assert_eq!(data.value, "more text", "second footnote value preserved");

    let para = arena[fn2].children[0];
    assert_eq!(
        count_type(&arena, para, SyntaxT::PlainText),
        1,
        "second footnote paragraph must contain its plain text"
    );
}
