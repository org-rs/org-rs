mod common;
use common::*;
use org_element::markup::strip_fixed_width_colons;

#[test]
fn basic() {
    for (input, desc) in &[
        (": Fixed width line\n", "basic"),
        ("  : Indented fixed width\n", "indented"),
        (
            "#+NAME: my-fixed\n: Fixed width line\n",
            "with name affiliated",
        ),
        (": Line 1\n: Line 2\n: Line 3\n", "multiple lines"),
    ] {
        let count = get_type_count(input, SyntaxT::FixedWidth, ParseGranularity::Element);
        assert_eq!(
            count, 1,
            "Expected 1 fixed width ({}), found {}",
            desc, count
        );
    }
}

#[test]
fn multiline_value_strips_colons() {
    let input = ": Line 1\n: Line 2\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let fw_node = section_children.first().expect("Expected fixed-width node");

    let Syntax::FixedWidth(raw) = &arena[*fw_node].data else {
        panic!("Expected FixedWidth, got: {:?}", arena[*fw_node].data);
    };
    let value = strip_fixed_width_colons(raw);
    assert!(
        !value.contains(':'),
        "Fixed-width value should not contain colons, got: {:?}",
        value
    );
    assert!(
        value.contains("Line 1"),
        "Fixed-width value should contain 'Line 1', got: {:?}",
        value
    );
    assert!(
        value.contains("Line 2"),
        "Fixed-width value should contain 'Line 2', got: {:?}",
        value
    );
}
