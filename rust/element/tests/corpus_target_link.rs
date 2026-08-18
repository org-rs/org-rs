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
//!
//! Corpus discrepancies for target and link objects.
//!
//! Discrepancies observed in emacs_china_org_manual.org:
//!   1. Radio target (`<<<target>>>`) not parsed — `try_parse_target` catches
//!      it as a regular `Target` with wrong content (includes the leading `<`).
//!   2. `[[#custom-id]]` links not recognised as `LinkType::CustomId` — they
//!      fall through to `Fuzzy` in `LinkData::new`.
//!   3. `[[url] ]` (space before closing `]]`) incorrectly parsed as a link —
//!      Emacs does not treat this as a valid bracket link.
//!   4. `[[#custom-id][description]]` links — not parsed at all.
use bumpalo::Bump;
use org_element::data::{LinkType, NodeArena, NodeId, Syntax, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};
use org_element::testutils::*;

fn find_first_link<'a>(arena: &NodeArena<'_, 'a>, id: NodeId) -> Option<NodeId> {
    if matches!(arena[id].data, Syntax::Link(_)) {
        return Some(id);
    }
    for &child in &arena[id].children {
        if let found @ Some(_) = find_first_link(arena, child) {
            return found;
        }
    }
    None
}

fn first_link_type(input: &str) -> Option<LinkType> {
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let id = find_first_link(&arena, root)?;
    if let Syntax::Link(d) = &arena[id].data {
        Some(d.link_type())
    } else {
        None
    }
}

/// `<<<radio-target>>>` must be parsed as a `RadioTarget` node, not a
/// `Target`.  Emacs org-element treats triple-angled brackets as a
/// distinct object type.
#[test]
fn radio_target_is_not_plain_target() {
    let input = "<<<radio-target>>>\n";
    let radio_count = get_type_count(input, SyntaxT::RadioTarget, ParseGranularity::Object);
    assert_eq!(
        radio_count, 1,
        "expected 1 RadioTarget for '<<<radio-target>>>', got {} — \
         try_parse_target should not consume radio-target syntax",
        radio_count
    );
}

/// `[[#custom-id]]` must be recognised as `LinkType::CustomId`.
///
/// Emacs distinguishes `#`-prefixed links inside `[...]` as custom-ID
/// references.  `LinkData::new` currently falls through to `Fuzzy`.
#[test]
fn custom_id_link_type() {
    let typ = first_link_type("[[#my-custom-id]]\n");
    assert_eq!(
        typ,
        Some(LinkType::CustomId),
        "[[#my-custom-id]] must have link type CustomId, got {:?}",
        typ
    );
}

/// `[[url] ]` (space inserted before the closing `]]`) is **not** a valid
/// Org bracket link per Emacs.  Rust's `try_parse_link` greedily matches
/// any pair of `]]` even when separated by whitespace.
#[test]
fn space_before_closing_brackets_is_not_a_link() {
    let input = "[[#my-custom-id] ](no space)\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 0,
        "expected 0 Links for '[[url] ]' (space before ]]), got {} — \
         space inside closing brackets is not a valid Org link",
        link_count
    );
}

/// `[[#custom-id][description]]` — a custom-ID link with a description —
/// must be parsed as a `Link` with `LinkType::CustomId`.
#[test]
fn custom_id_link_with_description() {
    let input = "[[#html-export][export to HTML]]\n";
    let typ = first_link_type(input);
    assert_eq!(
        typ,
        Some(LinkType::CustomId),
        "[[#html-export][export to HTML]] must have link type CustomId, got {:?}",
        typ
    );
}

/// A bracket-link description may itself contain a balanced `[...]` group.
/// Emacs parses `[[id:X][[Tag] rest]]` as a *single* link spanning the
/// whole construct: the `]` inside `[Tag]` closes its own `[`, not the
/// outer link.  `try_parse_link` must track bracket depth in the
/// description rather than stopping at the first `]]`.
///
/// Corpus origin: yantar92_config.org (id-links with `[Source]` prefixes).
#[test]
fn link_description_can_contain_balanced_brackets() {
    let input = "[[id:X][[Emacsconf] talks]] after\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 1,
        "expected exactly 1 Link for '[[id:X][[Emacsconf] talks]]', got {} — \
         balanced [..] inside the description must not break the link",
        link_count
    );

    // The single link must span the entire `[[id:X][[Emacsconf] talks]]`
    // (27 bytes) plus the trailing space absorbed as post-blank (= 28),
    // not stop early at the inner `]`.
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let id = find_first_link(&arena, root).expect("a Link node");
    let loc = arena[id].location;
    assert_eq!(
        (loc.start, loc.end),
        (0, 28),
        "link should span '[[id:X][[Emacsconf] talks]] ' (incl. post-blank), got {}..{}",
        loc.start,
        loc.end
    );
}

/// Corpus discrepancy: `chapter-appendix-f.org` @5208.
///
/// A bare (angle-less) plain link must not swallow a trailing ideographic
/// full stop `。` (U+3002).  In `...org-8.0.html。` Emacs ends the link
/// before the `。`, leaving it as plain text; org-rs greedily includes the
/// `。` (and even the following space) in the link path.
///
/// Emacs strips trailing punctuation that is not a valid link path char;
/// org-rs's plain-link scanner treats `。` as a path character.
#[test]
fn plain_link_excludes_trailing_ideographic_full_stop() {
    let input = "見るhttps://orgmode.org/x。 後\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let id = find_first_link(&arena, root).expect("expected a plain Link node");
    let loc = arena[id].location;
    let text = &input[loc.start..loc.end];
    assert!(
        !text.contains('。'),
        "plain link must stop before the ideographic full stop '。', \
         but link spans {:?}",
        text
    );
    assert_eq!(
        text, "https://orgmode.org/x",
        "plain link should be exactly the URL, got {:?}",
        text
    );
}

fn find_first_target(arena: &NodeArena, id: NodeId) -> Option<NodeId> {
    if matches!(arena[id].data, Syntax::Target(_)) {
        return Some(id);
    }
    for &child in &arena[id].children {
        if let found @ Some(_) = find_first_target(arena, child) {
            return found;
        }
    }
    None
}

fn first_target_value(input: &str) -> Option<String> {
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let id = find_first_target(&arena, root)?;
    if let Syntax::Target(v) = &arena[id].data {
        Some((*v).to_owned())
    } else {
        None
    }
}

/// `<<<My Target>> >` is *not* a radio target (no closing `>>>`).  Emacs
/// treats the leading `<` as plain text and parses `<<My Target>>` as a
/// regular `Target` starting at the **second** `<` with value `My Target`.
/// `try_parse_target` must reject content beginning with `<` so the
/// dispatch retries one byte later.
///
/// Corpus origin: emacs_china_org_manual.org @87116.
#[test]
fn target_triple_angle_falls_back_to_second_bracket() {
    let value = first_target_value("就<<<My Target>> >话\n");
    assert_eq!(
        value.as_deref(),
        Some("My Target"),
        "expected Target value 'My Target' (starting at 2nd '<'), got {:?}",
        value
    );
}

/// Target content may not contain `<` (Emacs `org-target-regexp` excludes
/// angle brackets).  `<<a<b>>` is plain text, not a target.
#[test]
fn target_content_with_angle_bracket_rejected() {
    let n = get_type_count("<<a<b>>\n", SyntaxT::Target, ParseGranularity::Object);
    assert_eq!(
        n, 0,
        "'<<a<b>>' must not be a Target (content has '<'), got {}",
        n
    );
}

/// Target content may not start with whitespace: `<< a>>` is plain text.
#[test]
fn target_content_leading_space_rejected() {
    let n = get_type_count("<< a>>\n", SyntaxT::Target, ParseGranularity::Object);
    assert_eq!(
        n, 0,
        "'<< a>>' must not be a Target (leading space), got {}",
        n
    );
}

/// Target content may not end with whitespace: `<<a >>` is plain text.
#[test]
fn target_content_trailing_space_rejected() {
    let n = get_type_count("<<a >>\n", SyntaxT::Target, ParseGranularity::Object);
    assert_eq!(
        n, 0,
        "'<<a >>' must not be a Target (trailing space), got {}",
        n
    );
}

/// Targets have no post-char restriction: `x<<tgt>>y` is a valid target
/// even though a word character follows the closing `>>`.
#[test]
fn target_has_no_post_char_requirement() {
    let value = first_target_value("x<<tgt>>y\n");
    assert_eq!(
        value.as_deref(),
        Some("tgt"),
        "'x<<tgt>>y' must parse Target 'tgt' regardless of following char, got {:?}",
        value
    );
}

// ──────────────────────────────────────────────
// Radio link tests (Class E)
// ──────────────────────────────────────────────

/// A `<<<target>>>` definition followed by a matching word in the text
/// must produce a radio `Link` node.
#[test]
fn radio_link_basic() {
    let input = "<<<target>>>\ntext with target in it.\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 1,
        "expected 1 radio Link matching 'target', got {}",
        link_count
    );
}

/// Radio links match case-insensitively.
#[test]
fn radio_link_case_insensitive() {
    let input = "<<<Target>>>\ntext with target in it.\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 1,
        "case-insensitive radio link match should produce 1 Link, got {}",
        link_count
    );
}

/// Radio links normalise whitespace runs in the target.
/// This was the specific pattern from `org-publish-html-tutorial.org`
/// where `<<<Special comment section>>>` matched "special comment section"
/// in text.
#[test]
fn radio_link_whitespace_normalized() {
    let input = concat!(
        "<<<Special   comment section>>>\n",
        "Now remove the special comment section from the output.\n",
    );
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 1,
        "whitespace-normalized radio link should produce 1 Link, got {}",
        link_count
    );
}

/// Radio links must respect word boundaries: the matched text cannot be
/// adjacent to alphanumeric characters or underscores.
#[test]
fn radio_link_word_boundary_after() {
    let input = "<<<target>>>\nsometargetword\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 0,
        "radio link should not match when target is embedded in a word, got {}",
        link_count
    );
}

#[test]
fn radio_link_word_boundary_before() {
    let input = "<<<target>>>\nXYtarget\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 0,
        "radio link should not match when preceded by a word char, got {}",
        link_count
    );
}

/// When a target appears non-adjacent to word chars on BOTH sides, the
/// radio link must still match.
#[test]
fn radio_link_word_boundary_both_sides() {
    let input = "<<<target>>>\nXtargetY\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 0,
        "radio link should not match when surrounded by word chars, got {}",
        link_count
    );
}

/// Radio link nodes store the matched text as an inner `PlainText` child,
/// matching Emacs' tree structure.
#[test]
fn radio_link_has_inner_plain_text() {
    let input = "<<<target>>>\nsome target text.\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    fn find_link(arena: &NodeArena, id: NodeId) -> Option<NodeId> {
        if matches!(arena[id].data, Syntax::Link(_)) {
            return Some(id);
        }
        for &child in &arena[id].children {
            if let found @ Some(_) = find_link(arena, child) {
                return found;
            }
        }
        None
    }

    let link_id = find_link(&arena, root).expect("expected a radio Link node");
    let children = &arena[link_id].children;
    assert_eq!(
        children.len(),
        1,
        "radio Link must have exactly 1 child, got {}",
        children.len()
    );
    assert!(
        matches!(arena[children[0]].data, Syntax::PlainText(_)),
        "radio Link child must be PlainText"
    );
}

/// Multiple occurrences of the same target in the text should each
/// produce a separate radio Link.
#[test]
fn radio_link_multiple_occurrences() {
    let input = "<<<target>>>\ntarget and target again.\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 2,
        "expected 2 radio Links for two occurrences, got {}",
        link_count
    );
}

/// A radio link must not match inside a `=code=` or `~code~` span.
#[test]
fn radio_link_not_inside_code_span() {
    let input = "<<<target>>>\n=target= and ~target~.\n";
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 0,
        "radio link should not match inside code spans, got {}",
        link_count
    );
}

/// Defining a radio target inside a code span should not register it.
#[test]
fn radio_target_inside_code_not_collected() {
    let input = "=<<<target>>>= is not a radio target, but target should not match.\n";
    // The `<<<target>>>` inside =...= is not collected, so "target" in
    // the text should remain plain text (no Link).
    let link_count = get_type_count(input, SyntaxT::Link, ParseGranularity::Object);
    assert_eq!(
        link_count, 0,
        "radio target inside code should not be collected, got {}",
        link_count
    );
    // No RadioTarget should be produced either.
    let rt_count = get_type_count(input, SyntaxT::RadioTarget, ParseGranularity::Object);
    assert_eq!(
        rt_count, 0,
        "no RadioTarget should be created from code-span content, got {}",
        rt_count
    );
}
