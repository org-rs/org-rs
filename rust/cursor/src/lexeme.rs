use crate::{Char, Interval, Addressable, Line};
use crate::metrics::Metric;

/// Lexeme is anything that represents a meaningful value to the parser (e.g. char, string).
/// Usually lexeme is delimited by:
/// - 2 metrics, e.g. [Char..Char) == char, word, sentence etc..
/// - beginning of input and a metric, e.g. [..NewlineMetric] == Line
/// - metric and end of input - char or line and the end of input
pub trait Lexeme {
    type Item;

    fn is_on(s: &str, offset: usize) -> bool;

    fn get_prev(s: &str, offset: usize) -> Option<Addressable<Self::Item>>;

    fn get_next(s: &str, offset: usize) -> Option<Addressable<Self::Item>>;

    fn find_next(s: &str, offset: usize) -> Option<Interval>;

    fn find_prev(s: &str, offset: usize) -> Option<Interval>;
}


impl Lexeme for Char {
    type Item = char;

    // Any valid offset is some char
    fn is_on(s: &str, offset: usize) -> bool {
        offset < s.len()
    }

    fn find_prev(s: &str, offset: usize) -> Option<Interval> {
        if let Some(beg) = Char::prev(s, offset) {
            Some(Interval {
                start: beg,
                end: beg,
            })
        } else {
            None
        }
    }

    fn get_prev(s: &str, offset: usize) -> Option<Addressable<Self::Item>> {
        if let Some(i) = Self::find_prev(s, offset) {
            s[i.start..].chars().next().map(|c| Addressable {
                value: c,
                address: i,
            })
        } else {
            None
        }
    }

    fn find_next(s: &str, offset: usize) -> Option<Interval> {
        if let Some(beg) = Char::next(s, offset) {
            Some(Interval {
                start: beg,
                end: beg,
            })
        } else {
            None
        }
    }

    fn get_next(s: &str, offset: usize) -> Option<Addressable<Self::Item>> {
        if let Some(i) = Self::find_next(s, offset) {
            s[offset..].chars().next().map(|c| Addressable {
                value: c,
                address: i,
            })
        } else {
            None
        }
    }
}


impl Lexeme for Line {
    // Allocates!
    type Item = String;

    // Any valid offset is some line
    fn is_on(s: &str, offset: usize) -> bool {
        offset <= s.len()
    }

    /// Finds the the previous line relative to the offset.
    /// If no fist NewlineMetric found - offset is already on first line
    /// If no second NewlineMetric found - previous is the first line
    fn find_prev(s: &str, offset: usize) -> Option<Interval> {
        let end = if Line::is_boundary(s, offset) {
            offset
        } else {
            match Line::prev(s, offset) {
                None => return None,
                Some(x) => x,
            }
        };

        let beg = match Line::prev(s, end) {
            None => 0,
            Some(x) => x,
        };

        return Some(Interval {
            start: beg,
            end: end, //
        });
    }

    fn get_prev(s: &str, offset: usize) -> Option<Addressable<Self::Item>> {
        if let Some(i) = Self::find_prev(s, offset) {
            return Some(Addressable {
                address: i,
                value: String::from(&s[i.start..i.end]),
            });
        } else {
            None
        }
    }

    /// Finds the the next line relative to the offset.
    /// If no fist NewlineMetric found - offset is already on last line
    /// If no second NewlineMetric found - next is the last line
    fn find_next(s: &str, offset: usize) -> Option<Interval> {
        let beg = if Line::is_boundary(s, offset) {
            offset
        } else {
            match Line::next(s, offset) {
                None => return None,
                Some(x) => x,
            }
        };

        let end = match Line::next(s, beg) {
            None => s.len(),
            Some(x) => x,
        };
        return Some(Interval {
            start: beg,
            end: end,
        });
    }

    fn get_next(s: &str, offset: usize) -> Option<Addressable<Self::Item>> {
        if let Some(i) = Self::find_next(s, offset) {
            return Some(Addressable {
                address: i,
                value: String::from(&s[i.start..i.end]),
            });
        } else {
            None
        }
    }
}
