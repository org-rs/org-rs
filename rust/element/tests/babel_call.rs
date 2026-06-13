mod common;
use common::*;

#[test]
fn babel_call() {
    for (input, desc) in &[
        ("#+CALL: test()\n", "uppercase"),
        ("#+call: test()\n", "case insensitive"),
        ("#+CALL: test[:results output]()\n", "with header"),
        ("#+CALL: test(n=4)\n", "with arguments"),
    ] {
        let count = get_type_count(input, SyntaxT::BabelCall, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 babel call ({}), found {}",
            desc, count
        );
    }
}
