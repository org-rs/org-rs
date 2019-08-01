#[macro_use]
extern crate lazy_static;
extern crate memchr;
extern crate regex;

pub mod cursor;
pub mod emacs;
pub mod lexeme;
pub mod metrics;
pub mod search;


use regex::Regex;

lazy_static! {
    pub static ref REGEX_EMPTY_LINE: Regex = Regex::new(r"^[ \t]*$").unwrap();
}

/// UTF32 Char
/// As metric - The address of UTF char is is the address of it's first byte
/// As Lexeme - [char]
pub struct Char;

/// As metric - the address of '\n' byte
/// As lexeme - string between BOF and '\n',
///             between 2 occurrences of '\n',
///             or '\n' and EOF
pub struct Line;

// Beginning of input
pub struct BOF;

// End of input
pub struct EOF;

pub struct Addressable<T> {
    pub value: T,
    pub address: Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: usize,
    pub end: usize,
}

impl Interval {
    pub fn new(start: usize, end: usize) -> Interval {
        Interval { start, end }
    }
}

pub struct Cursor<'a> {
    data: &'a str,
    pos: usize,
}

/// Checks if a regular expression can match multiple lines.
pub fn is_multiline_regex(regex: &str) -> bool {
    // regex characters that match line breaks
    // todo: currently multiline mode is ignored
    let multiline_indicators = vec![r"\n", r"\r", r"[[:space:]]"];

    multiline_indicators.iter().any(|&i| regex.contains(i))
}

/// Given the inital byte of a UTF-8 codepoint, returns the number of
/// bytes required to represent the codepoint.
/// RFC reference : https://tools.ietf.org/html/rfc3629#section-4
/// TODO maybe rename to len()
pub fn len_utf8_from_first_byte(b: u8) -> usize {
    match b {
        b if b < 0x80 => 1,
        b if b < 0xe0 => 2,
        b if b < 0xf0 => 3,
        _ => 4,
    }
}
