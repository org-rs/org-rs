use org_element::testutils::*;

// ── ** at BOL inside #+begin_src: should NOT form a src-block ─────
// When `** ` appears at BOL inside a src-block, the section boundary
// created by the headline prevents `#+end_src` from being found.
// Both Emacs and Rust agree: no src-block in this case.

#[test]
fn bol_double_star_no_src_block() {
    let input = "#+begin_src emacs-lisp\n** Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "** at BOL should prevent src-block (count={})",
        count
    );
}

#[test]
fn bol_triple_star_no_src_block() {
    let input = "#+begin_src emacs-lisp\n*** Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "*** at BOL should prevent src-block (count={})",
        count
    );
}

#[test]
fn bol_single_star_no_src_block() {
    let input = "#+begin_src emacs-lisp\n* Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "* at BOL should prevent src-block (count={})",
        count
    );
}

// ── Indented / escaped ** inside src-block: should succeed ────────
// These are NOT headlines, so the src-block forms correctly.

#[test]
fn bol_indented_no_break() {
    let input = "#+begin_src emacs-lisp\n  ** Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "Indented ** should NOT break src-block (count={})",
        count
    );
}

#[test]
fn bol_escaped_no_break() {
    let input = "#+begin_src emacs-lisp\n,** Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "Escaped ,** should NOT break src-block (count={})",
        count
    );
}

#[test]
fn bol_no_space_no_break() {
    let input = "#+begin_src emacs-lisp\n**Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 1,
        "** without space should NOT break src-block (count={})",
        count
    );
}

#[test]
fn bol_double_space_no_break() {
    let input = "#+begin_src emacs-lisp\n**  Agenda\n(message \"hello\")\n#+end_src\n";
    let count = get_type_count(input, SyntaxT::SrcBlock, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "**  with 2+ spaces still counts as headline (count={})",
        count
    );
}
