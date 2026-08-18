//! Tests documenting known corpus discrepancies between org-rs and Emacs.
//! See `rust/element/examples/corpus-check.rs` for the full oracle comparison.

use org_element::testutils::*;

// ── Form feed (\x0c) ─────────────────────────────────────────────
// Emacs' `org-element--blank-p` uses `[ \t]*$` for blank-line detection.
// Form feed (0x0C) is NOT in `[ \t]`, so `\x0c` is paragraph content.
// Rust's `parse_elements` uses `trim_ascii()` (parser.rs:496-500),
// which classifies 0x0C as whitespace and skips the line, splitting
// the paragraph and letting a following `:DRAWERNAME:` become a drawer.
#[test]
fn form_feed_not_blank_in_paragraph() {
    // Emacs' `[ \t]*$` blank-line check does NOT include `\x0c` (form feed).
    // A `\x0c` line must be absorbed into the surrounding paragraph.
    // Rust's element-level blank-line skip (parser.rs:496-500) was using
    // `trim_ascii()` which classifies `\x0c` as whitespace, splitting the paragraph.
    let input = "text\n\x0c\nmore text\n";
    let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "Emacs absorbs \\x0c into paragraph; Rust splits (count={})",
        count
    );
}

// ── Missing plain_text at section boundary ──────────────────────
// chapter-appendix-f.org: a plain_text node is offset by 3 bytes
// at a section boundary where a headline follows paragraph content.
#[test]
fn section_boundary_plain_text_offset() {
    let input = "text\n\n** headline\ncontent\n";
    let pt_count = get_type_count(input, SyntaxT::PlainText, ParseGranularity::Object);
    assert!(
        pt_count >= 2,
        "Expected at least 2 plain_text nodes; found {}",
        pt_count
    );
}

#[test]
fn paragraph_with_dash_prefix() {
    let input = "para1\n----- text\npara2\n";
    let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "Expected 1 paragraph (----- text is not a horizontal rule), found {}",
        count
    );
}

#[test]
fn paragraph_contains_list_item() {
    let input = "before\n- item\nafter\n";
    let list_count = get_type_count(input, SyntaxT::PlainList, ParseGranularity::Element);
    assert_eq!(
        list_count, 1,
        "Expected 1 list when '- item' is mid-paragraph, found {}",
        list_count
    );
}
