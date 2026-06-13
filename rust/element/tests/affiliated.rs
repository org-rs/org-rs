mod common;
use common::*;

#[test]
fn collects_all_affiliated_types() {
    let input = "#+NAME: my-name\n#+CAPTION: my caption\n#+ATTR_HTML: class=foo\n#+HEADER: :var x=1\n: fixed-width\n";
    let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "Expected 1 fixed-width with affiliated keywords, found {}",
        count
    );
}
