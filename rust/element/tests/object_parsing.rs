use org_element::data::LinkType;
use org_element::testutils::*;

#[test]
fn basic_markup() {
    for (input, typ) in &[
        ("*bold*\n", SyntaxT::Bold),
        ("/italic/\n", SyntaxT::Italic),
        ("_underline_\n", SyntaxT::Underline),
        ("+strikethrough+\n", SyntaxT::StrikeThrough),
        ("~code~\n", SyntaxT::Code),
        ("=verbatim=\n", SyntaxT::Verbatim),
    ] {
        let count = get_type_count(input, *typ, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 {:?} object, found {}", typ, count);
    }
}

#[test]
fn single_char_markup() {
    for (input, typ) in &[("*a*\n", SyntaxT::Bold), ("/a/\n", SyntaxT::Italic)] {
        let count = get_type_count(input, *typ, ParseGranularity::Object);
        assert_eq!(
            count,
            1,
            "Expected 1 {:?} from single-char input '{}', found {}",
            typ,
            input.trim(),
            count
        );
    }
}

#[test]
fn other_objects() {
    for (input, typ, desc) in &[
        ("\\alpha\n", SyntaxT::Entity, "entity"),
        ("[[https://example.com][link]]\n", SyntaxT::Link, "link"),
        ("<<target>>\n", SyntaxT::Target, "target"),
        ("<2023-12-31>\n", SyntaxT::Timestamp, "timestamp"),
    ] {
        let count = get_type_count(input, *typ, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 {} object, found {}", desc, count);
    }
}

/// Emacs org-element requires ISO date format YYYY-MM-DD.  Non-ISO
/// formats like the European DD-MM-YYYY (e.g. "<24-09-2011>") must NOT
/// be parsed as timestamps.
#[test]
fn non_iso_date_not_a_timestamp() {
    let ts = get_type_count(
        "<24-09-2011>\n",
        SyntaxT::Timestamp,
        ParseGranularity::Object,
    );
    assert_eq!(
        ts, 0,
        "DD-MM-YYYY '<24-09-2011>' must not be parsed as a Timestamp, got {}",
        ts
    );
}

#[test]
fn objects_after_punctuation() {
    for (input, typ, desc) in &[
        ("[[link]].\n", SyntaxT::Link, "link"),
        ("<<target>>.\n", SyntaxT::Target, "target"),
        ("<2023-12-31>.\n", SyntaxT::Timestamp, "timestamp"),
    ] {
        let count = get_type_count(input, *typ, ParseGranularity::Object);
        assert_eq!(count, 1, "Expected 1 {} before '.', found {}", desc, count);
    }
}

#[test]
fn timestamp_with_time_range() {
    let input = "<2023-12-01 10:15-11:30>\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();

    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let para = section_children.first().expect("Expected paragraph");
    let para_children = &arena[*para].children;
    let ts_node = para_children.first().expect("Expected timestamp");

    let Syntax::Timestamp(data) = &arena[*ts_node].data else {
        panic!("Expected Timestamp, got: {:?}", arena[*ts_node].data);
    };
    assert_eq!(
        data.hour_end,
        Some(11),
        "Expected hour_end 11 from time range 10:15-11:30, got {:?}",
        data.hour_end
    );
    assert_eq!(
        data.minute_end,
        Some(30),
        "Expected minute_end 30 from time range 10:15-11:30, got {:?}",
        data.minute_end
    );
}

#[test]
fn link_type_with_brackets() {
    let input = "[[https://example.com]]\n";
    let bump = Bump::new();
    let mut parser = Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    let root_children = &arena[root].children;
    let section = root_children.first().expect("Expected section");
    let section_children = &arena[*section].children;
    let para = section_children.first().expect("Expected paragraph");
    let para_children = &arena[*para].children;
    let link_node = para_children.first().expect("Expected link");
    let Syntax::Link(data) = &arena[*link_node].data else {
        panic!("Expected Link, got: {:?}", arena[*link_node].data);
    };
    assert!(
        matches!(data.link_type(), LinkType::File),
        "Expected LinkType::File for https link, got {:?}",
        data.link_type()
    );
}

// ── Bold with triple-asterisk overlap ──────────────────────────────
// writting_collection.org: `***হুমায়ুন...***` produces 3 nested
// Bold nodes in both Emacs and Rust (the third `*` at the content
// region boundary passes the pre-char check at object_region_start).
#[test]
fn bold_triple_asterisk_bangla() {
    let input = "***হুমায়ুন অথবা সাহিত্য বিষয়ে একটা র‍্যান্ট***\n";
    let bold_count = get_type_count(input, SyntaxT::Bold, ParseGranularity::Object);
    assert_eq!(
        bold_count, 3,
        "Expected 3 Bold for ***...*** (matching Emacs); Rust found {}",
        bold_count
    );
}

// ── Inline src block ──────────────────────────────────────────────
// 2021-07-31-citations.org: `src_lang{...}` must be parsed as InlineSrcBlock.
#[test]
fn inline_src_block() {
    let input = "call src_elisp{(message \"hi\")}\n";
    let count = get_type_count(input, SyntaxT::InlineSrcBlock, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "expected 1 InlineSrcBlock for `src_elisp{{...}}`, found {}",
        count
    );
}
