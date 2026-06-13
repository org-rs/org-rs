mod common;
use common::*;

#[test]
fn horizontal_rule_basic_and_with_spaces() {
    for (input, desc) in &[("-----\n", "basic"), ("   -----   \n", "with spaces")] {
        let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 horizontal rule ({}), found {}",
            desc, count
        );
    }
}

#[test]
fn horizontal_rule_after_para_before_list() {
    // "para\n\n------\n- item\n"
    // Emacs parses: paragraph, horizontal_rule (6 dashes), plain_list
    // Rust should produce the same structure.
    for granularity in &[ParseGranularity::Element, ParseGranularity::Object] {
        let input = "para\n\n------\n- item\n";
        let bump = Bump::new();
        let mut parser = Parser::new(input, *granularity, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();
        let hr_count = count_type(&arena, root, SyntaxT::HorizontalRule);
        let list_count = count_type(&arena, root, SyntaxT::PlainList);
        assert_eq!(
            hr_count, 1,
            "Expected 1 horizontal rule before list (granularity={:?}), found {}",
            granularity, hr_count
        );
        assert_eq!(
            list_count, 1,
            "Expected 1 plain list after horizontal rule (granularity={:?}), found {}",
            granularity, list_count
        );
    }
}

/// A horizontal rule directly after a non-blank paragraph line (no
/// intervening blank line) must still break the paragraph and be
/// recognised. Regression from `karl_voit_config.org`:
/// `about 24 miles per\n--------\n` — Emacs emits paragraph +
/// horizontal-rule, but Rust absorbs the dashes into the paragraph.
#[test]
fn horizontal_rule_directly_after_paragraph_line() {
    let input = "about 24 miles per\n--------\n\nAnswer\n";
    let count = get_type_count(input, SyntaxT::HorizontalRule, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "Expected 1 horizontal rule after a non-blank paragraph line, found {}",
        count
    );
}
