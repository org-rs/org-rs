//! Inline babel calls (`call_Func-Name(args)`) are not yet parsed.
//!
//! Corpus origin: `org-mode.org` (Bernt Hansen's Org Mode guide).
//! Emacs `org-element` recognises `call_Func-Name(args)` as an
//! `inline-babel-call` object.  The Rust parser has no handler for
//! the `c` dispatch byte in `parse_objects`, so `call_` falls through
//! to `PlainText`.  Worse, the `_` in `call_` is matched by
//! `try_parse_script` and consumed as a subscript, fragmenting the
//! text stream.
//!
//! Fix: implement `try_parse_inline_babel_call` in `parse_objects`
//! (matching `call_` prefix, function name, `(`, arguments, `)`).
//! The data type `InlineBabelCallData` already exists.

mod common;
use common::*;
use org_element::data::SyntaxT;
use org_element::parser::ParseGranularity;

#[test]
fn inline_babel_call_is_not_parsed() {
    let input = "call_org-mode-doc-version()\n";
    let count = get_type_count(input, SyntaxT::InlineBabelCall, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "expected 1 InlineBabelCall for 'call_org-mode-doc-version()', got {count} — \
         no handler for inline babel call syntax exists yet"
    );
}

#[test]
fn underscore_in_babel_call_is_not_subscript() {
    let input = "call_org-mode-doc-version()\n";
    let count = get_type_count(input, SyntaxT::Script, ParseGranularity::Object);
    assert_eq!(
        count, 0,
        "expected 0 Script nodes in 'call_org-mode-doc-version()', got {count} — \
         the _ in call_ is part of the babel-call syntax, not a subscript marker"
    );
}

#[test]
fn inline_babel_call_embedded_in_sentence() {
    let input = "version call_org-mode-doc-version() of this\n";
    let count = get_type_count(input, SyntaxT::InlineBabelCall, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "expected 1 InlineBabelCall for 'call_org-mode-doc-version()' in sentence, got {count}"
    );
}

#[test]
fn inline_babel_call_not_script_in_sentence_context() {
    let input = "version call_org-mode-doc-version() of this\n";
    let script_count = get_type_count(input, SyntaxT::Script, ParseGranularity::Object);
    let babel_count = get_type_count(input, SyntaxT::InlineBabelCall, ParseGranularity::Object);
    assert_eq!(
        (babel_count, script_count),
        (1, 0),
        "expected (1 InlineBabelCall, 0 Script) for babel call in sentence, got ({babel_count}, {script_count})"
    );
}
