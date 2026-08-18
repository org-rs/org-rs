use org_element::testutils::*;

#[test]
fn keyword() {
    for (input, desc) in &[
        ("#+KEYWORD: value\n", "basic"),
        ("#+keyword: value\n", "case insensitive"),
        ("#+KEYWORD:    spaced value\n", "spaced"),
        ("#+KEYWORD: value\nparagraph\n", "with following paragraph"),
    ] {
        let count = get_type_count(input, SyntaxT::Keyword, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 keyword ({}), found {}", desc, count);
    }
}

#[test]
fn keyword_edge_cases() {
    let count = get_type_count(
        "#+KEY: val\n#+EMPTY:\n#+COLONS: a::b::c\n#+UNICODE: café 标签\n",
        SyntaxT::Keyword,
        ParseGranularity::Element,
    );
    assert_eq!(count, 4, "Expected 4 keywords, found {}", count);
}
