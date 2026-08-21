use org_element::testutils::*;

#[test]
fn multiple_node_properties() {
    let input = ":PROPERTIES:\n:CUSTOM_ID: my-id\n:PRIORITY: A\n: tags: :foo:bar:\n:END:\n";
    let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
    assert_eq!(count, 3, "Expected 3 node properties, found {}", count);
}

/// `:END:` closes the drawer and must not be parsed as a `NodeProperty`
/// with key `"END"`.  Regression test: the content range previously
/// included the `:END:` line, causing `parse_node_property_line` to
/// emit a spurious extra property.
#[test]
fn end_marker_not_a_property() {
    let input = ":PROPERTIES:\n:ID: abc123\n:END:\n";
    let count = get_type_count(input, SyntaxT::NodeProperty, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        ":END: must not be parsed as a node property (got {count})"
    );
}
