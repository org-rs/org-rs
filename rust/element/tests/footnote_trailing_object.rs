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
use bumpalo::Bump;
use org_element::data::{Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use org_element::testutils::*;

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

/// An inline footnote reference `[fn::...]` containing a link `[[...]]`
/// must correctly match the closing bracket of the footnote (not the
/// link's closing brackets).
///
/// Before the fix, `try_parse_footnote_reference` used `text.find(']')` to
/// locate the closing bracket.  When the definition body contained a link
/// (e.g. `[[target]]`), the first `]` hit was the link's own bracket,
/// truncating the footnote and leaving trailing text (post-link `).]`)
/// as orphaned `PlainText` outside the reference.
///
/// Corpus origin: org-manual.org (multiple occurrences, 119 total
/// discrepancies in this class).
#[test]
fn inline_footnote_with_link_uses_correct_closing_bracket() {
    let input = "text [fn::See [[target]] and more] end\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    // Locate the footnote reference node.
    let root_children = &arena[root].children;
    let section = root_children.first().expect("section");
    let para = arena[*section].children.first().expect("paragraph");
    let objects = &arena[*para].children;

    let fn_ref = objects
        .iter()
        .find(|&&id| matches!(&arena[id].data, Syntax::FootnoteReference(_)))
        .expect("footnote reference must be found");

    // Verify its byte span covers the full `[fn::See [[target]] and more]`.
    let loc = &arena[*fn_ref].location;
    let post_blank = arena[*fn_ref].post_blank;
    assert_eq!(
        &input[loc.start..loc.end - post_blank],
        "[fn::See [[target]] and more]",
        "footnote reference must span from [fn:: to the correct closing ]"
    );

    // The footnote's inner content must contain a parsed link.
    let link_count = count_type(&arena, *fn_ref, SyntaxT::Link);
    assert_eq!(
        link_count, 1,
        "expected 1 link inside the footnote reference"
    );

    // The plain text " and more " after the link must also be present.
    let text_after = count_type(&arena, *fn_ref, SyntaxT::PlainText);
    assert!(text_after >= 2, "expected at least 2 PlainText children");
}

/// A `[fn::...]` containing a full link with description `[[uri][desc]]`.
#[test]
fn inline_footnote_with_link_and_description() {
    let input = "text [fn::See [[https://orgmode.org][Org mode]] more] end\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("section");
    let para = arena[*section].children.first().expect("paragraph");
    let objects = &arena[*para].children;

    let fn_ref = objects
        .iter()
        .find(|&&id| matches!(&arena[id].data, Syntax::FootnoteReference(_)))
        .expect("footnote reference must be found");

    let loc = &arena[*fn_ref].location;
    let post_blank = arena[*fn_ref].post_blank;
    assert_eq!(
        &input[loc.start..loc.end - post_blank],
        "[fn::See [[https://orgmode.org][Org mode]] more]",
        "footnote must span the full inline definition"
    );

    let link_count = count_type(&arena, *fn_ref, SyntaxT::Link);
    assert_eq!(link_count, 1, "expected 1 link inside footnote");
}

/// Multiple links inside a single `[fn::...]`.
#[test]
fn inline_footnote_with_multiple_links() {
    let input = "text [fn::See [[a]] and [[b]] here] end\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("section");
    let para = arena[*section].children.first().expect("paragraph");
    let objects = &arena[*para].children;

    let fn_ref = objects
        .iter()
        .find(|&&id| matches!(&arena[id].data, Syntax::FootnoteReference(_)))
        .expect("footnote reference must be found");

    let loc = &arena[*fn_ref].location;
    let post_blank = arena[*fn_ref].post_blank;
    assert_eq!(
        &input[loc.start..loc.end - post_blank],
        "[fn::See [[a]] and [[b]] here]",
        "footnote must span both links"
    );

    let link_count = count_type(&arena, *fn_ref, SyntaxT::Link);
    assert_eq!(link_count, 2, "expected 2 links inside footnote");
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
