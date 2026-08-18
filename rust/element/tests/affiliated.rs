use org_element::testutils::*;

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

/// Corpus discrepancy: `2021-07-31-citations.org` (18 occurrences).
///
/// An element preceded by *two or more* consecutive affiliated keyword
/// lines must remain a SINGLE element carrying all of them.  Here a link
/// paragraph is preceded by `#+attr_latex:` and `#+attr_html:`:
///
/// ```org
/// #+attr_latex: :width 0.4
/// #+attr_html: :class invertible
/// [[file:figures/zotero-export-library.png]]
/// ```
///
/// Emacs parses this as one paragraph (with both affiliated keywords)
/// containing the link.  org-rs instead splits the first keyword line off
/// into its own standalone paragraph and only attaches the *last* keyword
/// to the link paragraph — producing 2 paragraphs instead of 1.
#[test]
fn multiple_affiliated_keywords_stay_with_one_element() {
    let input =
        "#+attr_latex: :width 0.4\n#+attr_html: :class invertible\n[[file:figures/x.png]]\n";
    let count = get_type_count(input, SyntaxT::Paragraph, ParseGranularity::Object);
    assert_eq!(
        count, 1,
        "two affiliated keywords + a link must be 1 paragraph, found {} \
         (the first `#+attr_latex:` line is being orphaned into its own paragraph)",
        count
    );
}
