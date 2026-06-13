mod common;
use common::*;

#[test]
fn footnote_definition() {
    for (input, desc) in &[
        ("[fn:1] This is a footnote.\n", "numbered"),
        ("[fn:my-note] This is a named footnote.\n", "named"),
        ("[fn:1]\nMulti-line\nfootnote content\n", "multiline"),
    ] {
        let count = get_type_count(
            input,
            SyntaxT::FootnoteDefinition,
            ParseGranularity::Element,
        );
        assert_eq!(
            count, 1,
            "Expected 1 footnote definition ({}), found {}",
            desc, count
        );
    }
}

#[test]
fn content_location() {
    let input = "[fn:1] This is footnote content\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected at least one child");
    let section_children = &arena[*section].children;
    let fn_node = section_children
        .first()
        .expect("Expected footnote in section");

    let Syntax::FootnoteDefinition(_) = &arena[*fn_node].data else {
        panic!(
            "Expected FootnoteDefinition, got: {:?}",
            arena[*fn_node].data
        );
    };
    let content_loc = &arena[*fn_node]
        .content_location
        .expect("Footnote definition should have content_location");
    assert_eq!(
        content_loc.start, 7,
        "contents-begin should be after [fn:1] and space"
    );
    let content = &input[content_loc.start..content_loc.end];
    // Emacs' :contents-end includes the final newline of the last content
    // line, so the interval carries the trailing "\n".
    assert_eq!(content, "This is footnote content\n", "content should match");
}

#[test]
fn with_label() {
    let input = "[fn:my-label] Some content\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected at least one child");
    let section_children = &arena[*section].children;
    let fn_node = section_children
        .first()
        .expect("Expected footnote in section");

    let Syntax::FootnoteDefinition(data) = &arena[*fn_node].data else {
        panic!("Expected FootnoteDefinition syntax type");
    };
    assert_eq!(data.label, "my-label", "Label should be extracted");
    assert_eq!(data.value, "Some content", "Value should be raw text");
}

#[test]
fn footnote_reference_inline() {
    let input = "Paragraph with reference[fn:1] and another[fn:inline:inline note]\n";
    let count = get_type_count(input, SyntaxT::FootnoteReference, ParseGranularity::Object);
    assert_eq!(count, 2, "Expected 2 footnote references, found {}", count);
}

/// Two consecutive footnote definitions (no blank line) MUST be
/// siblings inside the section, NOT nested.
///
/// Emacs behaviour (org-element.el lines 1085-1147):
///   The `end` of each footnote is set by searching forward for
///   `org-element--footnote-separator`, which matches
///   `[fn:...` (the next footnote def), a headline, or 2+ blank
///   lines.  When a new `[fn:` is found, Emacs backs up one line
///   and sets `end` to the beginning of that line — so each
///   footnote owns only its own content.
///
/// Rust bug (markup.rs ~lines 212-244):
///   The scanning loop breaks when it sees the next `[fn:` or `* `
///   but *without updating* `end` from its initial `line_end_pos`.
///   The fallback on line 241 then sets `end = limit`, causing the
///   first footnote to swallow the second as nested content.
#[test]
fn back_to_back_footnotes_are_siblings_not_nested() {
    // Two footnotes, no blank line between them.
    let input = "[fn:1] First footnote content\n\
                  [fn:2] Second footnote content\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    // Find the section node (only child of root).
    let root_children = &arena[root].children;
    assert_eq!(
        root_children.len(),
        1,
        "root should have exactly one section"
    );
    let section = root_children[0];
    let section_children = &arena[section].children;

    // There must be exactly two direct children of the section, both
    // FootnoteDefinitions (as flat siblings).
    assert_eq!(
        section_children.len(),
        2,
        "section should have exactly 2 direct children, found {} — \
         this means the first footnote swallowed the second",
        section_children.len()
    );

    let fn1_id = section_children[0];
    let fn2_id = section_children[1];

    let Syntax::FootnoteDefinition(data1) = &arena[fn1_id].data else {
        panic!("first child should be FootnoteDefinition");
    };
    let Syntax::FootnoteDefinition(data2) = &arena[fn2_id].data else {
        panic!("second child should be FootnoteDefinition");
    };

    assert_eq!(data1.label, "1", "first footnote label");
    assert_eq!(data2.label, "2", "second footnote label");
    assert_eq!(
        data1.value, "First footnote content",
        "first footnote value"
    );
    assert_eq!(
        data2.value, "Second footnote content",
        "second footnote value"
    );

    // The first footnote's end should NOT extend past the start of
    // the second footnote (i.e., they must not overlap).
    assert!(
        arena[fn1_id].location.end <= arena[fn2_id].location.start,
        "first footnote end ({}) must not exceed second footnote start ({})",
        arena[fn1_id].location.end,
        arena[fn2_id].location.start,
    );
}

/// Three back-to-back footnotes: same constraint, all siblings.
#[test]
fn three_back_to_back_footnotes_are_siblings() {
    let input = "[fn:a] Note A\n\
                  [fn:b] Note B\n\
                  [fn:c] Note C\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children[0];
    let section_children = &arena[section].children;

    assert_eq!(
        section_children.len(),
        3,
        "expected 3 sibling footnote definitions, found {}",
        section_children.len()
    );

    for (i, &child) in section_children.iter().enumerate() {
        let Syntax::FootnoteDefinition(data) = &arena[child].data else {
            panic!("child {} should be FootnoteDefinition", i);
        };
        assert_eq!(data.label, ["a", "b", "c"][i], "label mismatch at {}", i);
    }

    // Verify no overlap between adjacent footnotes.
    for i in 0..section_children.len() - 1 {
        let end = arena[section_children[i]].location.end;
        let next_start = arena[section_children[i + 1]].location.start;
        assert!(
            end <= next_start,
            "footnote {} end ({}) overlaps footnote {} start ({})",
            i,
            end,
            i + 1,
            next_start
        );
    }
}

/// Two footnotes separated by a blank line must be siblings,
/// not nested.  Reproduces a corpus bug where the blank-line
/// terminator sets `end = search_pos` but `search_pos` still
/// equals `line_end_pos`, so the post-loop fallback
/// (`if end == line_end_pos { end = limit; }`) incorrectly fires
/// and the first footnote swallows the rest of the buffer.
#[test]
fn footnotes_separated_by_blank_line_are_siblings() {
    let input = "[fn:1] First.\n\n[fn:2] Second.\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    assert_eq!(
        root_children.len(),
        1,
        "root should have exactly one section"
    );
    let section = root_children[0];
    let section_children = &arena[section].children;

    assert_eq!(
        section_children.len(),
        2,
        "section should have exactly 2 direct children (both footnote \
         definitions as siblings), found {} — the first footnote is \
         swallowing the second because the blank-line terminator does \
         not survive the `end == line_end_pos` fallback",
        section_children.len()
    );

    let Syntax::FootnoteDefinition(data1) = &arena[section_children[0]].data else {
        panic!("first child should be FootnoteDefinition");
    };
    let Syntax::FootnoteDefinition(data2) = &arena[section_children[1]].data else {
        panic!("second child should be FootnoteDefinition");
    };
    assert_eq!(data1.label, "1");
    assert_eq!(data2.label, "2");
    assert_eq!(data1.value, "First.");
    assert_eq!(data2.value, "Second.");
}

/// A footnote followed by a headline: the footnote must NOT
/// swallow the headline.
#[test]
fn footnote_before_headline_is_not_nested() {
    let input = "[fn:1] A footnote\n\
                  * A headline\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Element, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    // root should have section + headline
    assert!(
        root_children.len() >= 2,
        "expected at least 2 root children (section + headline), found {}",
        root_children.len()
    );

    let section = root_children[0];
    let section_children = &arena[section].children;

    assert_eq!(
        section_children.len(),
        1,
        "section should have exactly 1 child (the footnote)"
    );

    let Syntax::FootnoteDefinition(data) = &arena[section_children[0]].data else {
        panic!("section child should be FootnoteDefinition");
    };
    assert_eq!(data.label, "1");

    // The headline should be a sibling of the section, not nested
    // inside the footnote.
    let headline = root_children[1];
    let Syntax::Headline(_) = &arena[headline].data else {
        panic!(
            "second root child should be Headline, got: {:?}",
            arena[headline].data
        );
    };
}
