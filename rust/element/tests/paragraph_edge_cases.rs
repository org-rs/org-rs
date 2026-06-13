mod common;
use common::*;

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
