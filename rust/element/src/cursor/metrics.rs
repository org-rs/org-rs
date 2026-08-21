//    This file is part of org-rs.
//
//    org-rs is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    org-rs is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with org-rs.  If not, see <https://www.gnu.org/licenses/>.

use memchr::{memchr, memrchr};

pub trait Metric {
    fn is_boundary(s: &str, offset: usize) -> bool;
    fn prev(s: &str, offset: usize) -> Option<usize>;
    fn next(s: &str, offset: usize) -> Option<usize>;
}

pub struct BaseMetric(());

impl Metric for BaseMetric {
    #[inline]
    fn is_boundary(s: &str, offset: usize) -> bool {
        s.is_char_boundary(offset)
    }

    #[inline]
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

    #[inline]
    fn next(s: &str, offset: usize) -> Option<usize> {
        if offset == s.len() {
            None
        } else {
            let b = s.as_bytes()[offset];
            Some(offset + len_utf8_from_first_byte(b))
        }
    }
}

pub struct LinesMetric(());

impl Metric for LinesMetric {
    #[inline]
    fn is_boundary(s: &str, offset: usize) -> bool {
        if offset == 0 {
            false
        } else {
            s.as_bytes()[offset - 1] == b'\n'
        }
    }

    #[inline]
    fn prev(s: &str, offset: usize) -> Option<usize> {
        if offset == 0 {
            return None;
        }
        memrchr(b'\n', &s.as_bytes()[..offset - 1]).map(|pos| pos + 1)
    }

    #[inline]
    fn next(s: &str, offset: usize) -> Option<usize> {
        memchr(b'\n', &s.as_bytes()[offset..]).map(|pos| offset + pos + 1)
    }
}

/// Given the initial byte of a UTF-8 codepoint, returns the number of
/// bytes required to represent the codepoint.
/// RFC reference: https://tools.ietf.org/html/rfc3629#section-4
#[inline]
fn len_utf8_from_first_byte(b: u8) -> usize {
    match b {
        b if b < 0x80 => 1,
        b if b < 0xe0 => 2,
        b if b < 0xf0 => 3,
        _ => 4,
    }
}
