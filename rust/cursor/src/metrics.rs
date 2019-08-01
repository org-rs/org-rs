use crate::{len_utf8_from_first_byte, Char, Line, BOF, EOF};
use memchr::{memchr, memrchr};

/// Metric is an address of special kind of marker.
/// Metric by itself does represent a user-facing value (e.g. char, string..)
pub trait Metric {
    /// Is this metric located by given offset in a given string
    fn is_boundary(s: &str, offset: usize) -> bool;

    /// Try to find previous metric relative the given offset in a given string
    fn prev(s: &str, offset: usize) -> Option<usize>;

    /// Try to find next metric relative the given offset in a given string
    fn next(s: &str, offset: usize) -> Option<usize>;

    fn at_or_next(s: &str, offset: usize) -> Option<usize> {
        if Self::is_boundary(s, offset) {
            Some(offset)
        } else {
            Self::next(s, offset)
        }
    }

    fn at_or_prev(s: &str, offset: usize) -> Option<usize> {
        if Self::is_boundary(s, offset) {
            Some(offset)
        } else {
            Self::prev(s, offset)
        }
    }
}

impl Metric for Char {
    fn is_boundary(s: &str, offset: usize) -> bool {
        s.is_char_boundary(offset)
    }

    fn prev(s: &str, offset: usize) -> Option<usize> {
        if offset == 0 {
            None
        } else {
            let mut len = 1;
            while !s.is_char_boundary(offset - len) {
                len += 1;
            }
            Some(offset - len)
        }
    }

    fn next(s: &str, offset: usize) -> Option<usize> {
        if offset == s.len() {
            None
        } else {
            let b = s.as_bytes()[offset];
            Some(offset + len_utf8_from_first_byte(b))
        }
    }
}

impl Metric for Line {
    fn is_boundary(s: &str, offset: usize) -> bool {
        if offset == 0 {
            false
        } else {
            s.as_bytes()[offset - 1] == b'\n'
        }
    }

    fn prev(s: &str, offset: usize) -> Option<usize> {
        debug_assert!(offset > 0, "caller is responsible for validating input");
        memrchr(b'\n', &s.as_bytes()[..offset - 1]).map(|pos| pos + 1)
    }

    fn next(s: &str, offset: usize) -> Option<usize> {
        memchr(b'\n', &s.as_bytes()[offset..]).map(|pos| offset + pos + 1)
    }
}

impl Metric for BOF {
    fn is_boundary(_s: &str, offset: usize) -> bool {
        offset == 0
    }

    fn prev(s: &str, offset: usize) -> Option<usize> {
        if Self::is_boundary(s, offset) {
            None
        } else {
            Some(0)
        }
    }

    fn next(_s: &str, _offset: usize) -> Option<usize> {
        None
    }
}

impl Metric for EOF {
    fn is_boundary(s: &str, offset: usize) -> bool {
        offset == s.len()
    }

    fn prev(_s: &str, _offset: usize) -> Option<usize> {
        None
    }

    fn next(s: &str, offset: usize) -> Option<usize> {
        if Self::is_boundary(s, offset) {
            None
        } else {
            Some(s.len())
        }
    }
}
