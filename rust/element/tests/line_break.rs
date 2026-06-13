mod common;
use common::*;

/// `text \\` at end of a line is an Org line break; the paragraph
/// continues on the next line. From the Yantar92 corpus (LOGBOOK entries
/// like `[2019-12-18 Wed 16:27] \\`).
#[test]
fn line_break_at_end_of_line() {
    let count = get_type_count(
        "first \\\\\nsecond\n",
        SyntaxT::LineBreak,
        ParseGranularity::Object,
    );
    assert_eq!(count, 1, "expected 1 LineBreak, found {}", count);
}

/// A longer backslash run or a mid-line `\\` (followed by non-whitespace)
/// is not a line break.
#[test]
fn non_line_break_backslashes() {
    assert_eq!(
        get_type_count(
            "a\\\\\\\\\nx\n",
            SyntaxT::LineBreak,
            ParseGranularity::Object
        ),
        0,
        "four backslashes are not a line break"
    );
    assert_eq!(
        get_type_count("a \\\\ b\n", SyntaxT::LineBreak, ParseGranularity::Object),
        0,
        "mid-line `\\\\` followed by text is not a line break"
    );
}
