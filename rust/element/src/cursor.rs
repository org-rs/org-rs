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

// Parts of the cursor code are shamelessly copied from xi-rope
// https://github.com/xi-editor/xi-editor/tree/master/rust/rope

use memchr::{memchr, memrchr};
use regex::{Captures, Match, Regex};

use crate::headline::REGEX_HEADLINE_MULTILINE;

lazy_static! {
    pub static ref REGEX_EMPTY_LINE: CachedRegex =
        CachedRegex::new(Regex::new(r"^[ \t]*$").unwrap());
}

/// A [`Regex`] with its multiline-ness precomputed.
///
/// The `multiline` flag is computed once at construction time by checking
/// whether the pattern contains `\n`, `\r`, or `[[:space:]]`.  This avoids
/// re-scanning the pattern string on every call to [`Cursor::looking_at`]
/// or [`Cursor::capturing_at`].
#[derive(Debug, Clone)]
pub struct CachedRegex {
    regex: Regex,
    multiline: bool,
}

impl CachedRegex {
    /// Build a `CachedRegex` from a `Regex`, precomputing whether it can
    /// match across line boundaries.
    #[inline]
    pub fn new(re: Regex) -> Self {
        let multiline = is_multiline_regex(re.as_str());
        CachedRegex {
            regex: re,
            multiline,
        }
    }

    /// `true` when the wrapped pattern can match across multiple lines.
    #[inline]
    pub fn multiline(&self) -> bool {
        self.multiline
    }
}

impl std::ops::Deref for CachedRegex {
    type Target = Regex;
    #[inline]
    fn deref(&self) -> &Regex {
        &self.regex
    }
}

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
        debug_assert!(offset > 0, "caller is responsible for validating input");
        memrchr(b'\n', &s.as_bytes()[..offset - 1]).map(|pos| pos + 1)
    }

    #[inline]
    fn next(s: &str, offset: usize) -> Option<usize> {
        memchr(b'\n', &s.as_bytes()[offset..]).map(|pos| offset + pos + 1)
    }
}

pub struct Cursor<'a> {
    data: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    #[inline]
    pub fn new(data: &'a str, pos: usize) -> Cursor<'a> {
        Cursor { data, pos }
    }

    #[inline]
    pub fn set(&mut self, pos: usize) {
        self.pos = pos;
    }

    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Get next codepoint after cursor position, and advance cursor.
    #[inline]
    pub fn get_next_char(&mut self) -> Option<char> {
        let pos = self.pos;
        if let Some(offset) = self.next::<BaseMetric>() {
            self.pos = offset;
            self.data[pos..].chars().next()
        } else {
            None
        }
    }

    /// Get previous codepoint before cursor position, and advance cursor backwards.
    #[inline]
    pub fn get_prev_char(&mut self) -> Option<char> {
        if let Some(offset) = self.prev::<BaseMetric>() {
            self.pos = offset;
            self.data[offset..].chars().next()
        } else {
            None
        }
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next<M: Metric>(&mut self) -> Option<usize> {
        if let Some(offset) = M::next(self.data, self.pos) {
            self.pos = offset;
            Some(offset)
        } else {
            None
        }
    }

    #[inline]
    pub fn is_boundary<M: Metric>(&self) -> bool {
        M::is_boundary(self.data, self.pos)
    }

    #[inline]
    pub fn prev<M: Metric>(&mut self) -> Option<usize> {
        if let Some(offset) = M::prev(self.data, self.pos) {
            self.pos = offset;
            Some(offset)
        } else {
            None
        }
    }

    #[inline]
    pub fn at_or_next<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.next::<M>()
        }
    }

    #[inline]
    pub fn at_or_prev<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.prev::<M>()
        }
    }

    /// Count the number of boundaries between two points.
    #[inline]
    pub fn count_between<M: Metric>(&self, begin: usize, end: usize) -> usize {
        let mut cur = Cursor { ..*self };
        cur.set(begin);
        let mut cnt = 0;
        if cur.is_boundary::<M>() {
            cnt += 1;
        }
        while cur.pos() < end {
            cur.next::<M>();
            cnt += 1;
        }
        cnt
    }

    /// Skip over space, tabs and newline characters
    /// Cursor position is set before next non-whitespace char
    #[inline]
    pub fn skip_whitespace(&mut self) -> usize {
        let bytes = self.data.as_bytes();
        while self.pos < bytes.len() {
            match bytes[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
        self.pos
    }

    /// Skip over space, tabs and newline characters, going backwards
    /// Cursor position is set before the previous non-whitespace char
    #[inline]
    pub fn skip_whitespace_backwards(&mut self) -> usize {
        let bytes = self.data.as_bytes();
        // Don't move if current position is a non-whitespace byte.
        if self.pos < bytes.len() && !matches!(bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
            return self.pos;
        }
        while self.pos > 0 {
            self.pos -= 1;
            if !matches!(bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
                break;
            }
        }
        self.pos
    }

    /// Moves cursor to the beginning of the current line.
    /// Acts like "Home" button
    /// If cursor is already at the beginning of the line - nothing happens
    /// Returns the position of the cursor
    #[inline]
    pub fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 && self.at_or_prev::<LinesMetric>().is_none() {
            self.set(0);
        }
        self.pos()
    }

    /// Moves cursor to the beginning of the next line. If there is no next line
    /// cursor position is set to len() of the input
    #[inline]
    pub fn goto_next_line(&mut self) -> usize {
        let res = self.next::<LinesMetric>();
        match res {
            None => {
                self.set(self.data.len());
                self.data.len()
            }
            Some(x) => x,
        }
    }

    /// Moves cursor to the beginning of the previous line.
    /// If there is no previous line then cursor position
    /// is set the beginning of the rope - 0
    #[inline]
    pub fn goto_prev_line(&mut self) -> usize {
        // move to the beginning of the current line
        self.goto_line_begin();
        if self.pos() == 0 {
            return 0;
        }
        let res = self.prev::<LinesMetric>();

        match res {
            None => {
                self.set(0);
                0
            }
            Some(x) => x,
        }
    }

    /// Return the character position of the first character on the current line.
    /// If N is none then acts as `goto_line_begin`
    /// Otherwise moves forward N - 1 lines first.
    /// with N < 1 cursor will move to previous lines
    ///
    /// Corresponds to `line-beginning-position` in elisp
    /// This function does not move the cursor (does save-excursion)
    #[inline]
    pub fn line_beginning_position(&mut self, n: Option<i32>) -> usize {
        let pos = self.pos();
        match n {
            None | Some(1) => {
                self.goto_line_begin();
            }

            Some(x) => {
                if x > 1 {
                    for _p in 0..x - 1 {
                        self.goto_next_line();
                    }
                } else {
                    self.goto_line_begin();
                    if self.pos() != 0 {
                        for _p in 0..(x - 1).abs() {
                            if self.prev::<LinesMetric>().is_none() {
                                self.set(0);
                                break;
                            }
                        }
                    }
                }
            }
        }

        let result = self.pos();
        self.set(pos);
        result
    }

    /// Return the character position of the last character on the current line.
    /// With argument N not nil or 1, move forward N - 1 lines first.
    /// If scan reaches end of buffer, return that position.
    ///
    /// Corresponds to `line-end-position` in elisp
    /// This function does not move the cursor (does save-excursion)
    #[inline]
    pub fn line_end_position(&mut self, n: Option<i32>) -> usize {
        let pos = self.pos();
        match n {
            None | Some(1) => {
                self.goto_next_line();
            }

            Some(x) => {
                if x > 1 {
                    for _p in 0..x {
                        self.goto_next_line();
                    }
                } else if self.pos() != 0 {
                    for _p in 0..=x.abs() {
                        if self.prev::<LinesMetric>().is_none() {
                            break;
                        }
                    }
                }
            }
        }

        let result = self.prev::<BaseMetric>().unwrap_or(0);
        self.set(pos);
        result
    }

    #[inline]
    pub fn char_after(&mut self, offset: usize) -> Option<char> {
        let pos = self.pos();
        self.set(offset);
        let result = self.get_next_char();
        self.set(pos);
        result
    }

    /// Checks if current line matches a given regex
    /// This function determines whether the text in
    /// the current buffer directly following cursor matches
    /// the regular expression regexp.
    /// “Directly following” means precisely that:
    /// the search is “anchored” and it can succeed only
    /// starting with the first character following point.
    /// The result is true if so, false otherwise.
    /// This function does not move cursor
    /// Use `capturing_at` if you need capture groups.
    #[inline]
    pub fn looking_at(&self, re: &CachedRegex) -> Option<Match<'a>> {
        let end = if !re.multiline() {
            LinesMetric::next(self.data, self.pos)
                .map(|p| p - 1) // exclude '\n' from the string'
                .unwrap_or_else(|| self.data.len())
        } else {
            self.data.len()
        };
        // Emacs `looking-at` is anchored at point: only accept
        // matches that start at position 0 in the searched substring.
        re.find(&self.data[self.pos..end])
            .filter(|m| m.start() == 0)
    }

    /// Acts exactly as `looking_at` but returns Captures
    /// This is slower than simple regex search so if you don't need
    /// capture groups use `looking_at` for better performance
    #[inline]
    pub fn capturing_at(&self, re: &CachedRegex) -> Option<Captures<'a>> {
        let end = if !re.multiline() {
            LinesMetric::next(self.data, self.pos)
                .map(|p| p - 1) // exclude '\n' from the string'
                .unwrap_or_else(|| self.data.len())
        } else {
            self.data.len()
        };

        re.captures(&self.data[self.pos..end])
    }

    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    /// If next headline is found returns it's start position
    #[inline]
    pub fn next_headline(&mut self) -> Option<usize> {
        // make sure we don't match current headline
        self.next::<LinesMetric>();
        let beg = self.pos();
        match REGEX_HEADLINE_MULTILINE.find(&self.data[beg..]) {
            Some(p) => {
                self.pos = beg + p.start();
                Some(beg + p.start())
            }
            None => None,
        }
    }

    /// Return true if cursor is on a headline.
    /// Corresponds to `org-at-heading-p`.
    ///
    /// Implemented as a direct byte check: find the start of the current line
    /// (using `memrchr` if not already at BOL), count leading `*` bytes, then
    /// verify the next byte is a space or tab.  No regex, no cursor mutation.
    #[inline]
    pub fn on_headline(&self) -> bool {
        let bytes = self.data.as_bytes();
        let line_start = if self.is_bol() {
            self.pos
        } else {
            memrchr(b'\n', &bytes[..self.pos]).map_or(0, |i| i + 1)
        };
        let tail = &bytes[line_start..];
        let n = tail.iter().take_while(|&&b| b == b'*').count();
        n > 0 && tail.get(n).map_or(false, |&b| b == b' ' || b == b'\t')
    }

    #[inline]
    pub fn is_bol(&self) -> bool {
        if self.pos == 0 {
            true
        } else {
            LinesMetric::is_boundary(self.data, self.pos)
        }
    }

    /// Search forward from point to str. Sets point to the end of the
    /// occurence found and returns point. bound is a position in the
    /// buffer. The match found must not end after that position. If
    /// None then search to end of the buffer. If count is specified,
    /// find the countth occurence. If countth occurence is not found
    /// None is returned. If count is not provided then 1 is used as
    /// count. Note that searching backward is not supported like it
    /// is in the elisp equivalent.
    #[inline]
    pub fn search_forward(
        &mut self,
        str: &str,
        bound: Option<usize>,
        count: Option<usize>,
    ) -> Option<usize> {
        let count = count.unwrap_or(1);

        let bound = match bound {
            Some(bound) => bound,
            _ => self.data.len(),
        };

        let pos = self.pos();
        if bound < pos {
            return None;
        }

        let mut iter = self.data[pos..].match_indices(str);
        let mut i = 1;
        loop {
            match iter.next() {
                Some(result) => {
                    if result.0 + pos + str.len() > bound {
                        return None;
                    }

                    if count == i {
                        self.set(result.0 + pos + str.len());
                        return Some(result.0 + pos + str.len());
                    }

                    i += 1;
                }
                None => return None,
            }
        }
    }

    /// (re-search-forward REGEXP &optional BOUND NOERROR COUNT)
    ///
    /// Search forward from point for regular expression REGEXP.
    /// Set point to the end of the occurrence found, and return point.
    /// The optional second argument BOUND is a buffer position that bounds
    ///   the search.  The match found must not end after that position.  A
    ///   value of nil means search to the end of the accessible portion of
    ///   the buffer.
    #[inline]
    pub fn re_search_forward(&mut self, re: &Regex, bound: Option<usize>) -> Option<usize> {
        let end = bound.unwrap_or(self.data.len());

        if end <= self.pos {
            return None;
        }

        // Set point to the end of the occurrence found, and return point.
        match re.find(&self.data[self.pos..end]) {
            None => None,
            Some(m) => {
                self.set(self.pos + m.end());
                Some(self.pos)
            }
        }
    }

    /// Moves point forward, stopping before a char not in str, or at position limit.
    #[inline]
    pub fn skip_chars_forward(&mut self, str: &str, limit: Option<usize>) -> usize {
        let pos = self.pos();
        let limit = match limit {
            Some(lim) => lim,
            _ => self.data.len(),
        };

        if pos >= limit {
            return 0;
        }

        let mut count = 0;
        while let Some(c) = self.get_next_char() {
            if !str.contains(c) {
                self.get_prev_char();
                return count;
            }
            if count + pos > limit {
                self.get_prev_char();
                return count;
            }
            count += 1;
        }
        count
    }
}

/// Given the inital byte of a UTF-8 codepoint, returns the number of
/// bytes required to represent the codepoint.
/// RFC reference : https://tools.ietf.org/html/rfc3629#section-4
#[inline]
pub fn len_utf8_from_first_byte(b: u8) -> usize {
    match b {
        b if b < 0x80 => 1,
        b if b < 0xe0 => 2,
        b if b < 0xf0 => 3,
        _ => 4,
    }
}

/// Checks if a regular expression can match multiple lines.
#[inline]
pub fn is_multiline_regex(regex: &str) -> bool {
    const MULTILINE_INDICATORS: &[&str] = &[r"\n", r"\r", r"[[:space:]]"];
    MULTILINE_INDICATORS.iter().any(|i| regex.contains(i))
}

#[cfg(test)]
mod test {

    use super::Cursor;
    use super::LinesMetric;
    use super::REGEX_EMPTY_LINE;

    use crate::cursor::BaseMetric;
    use crate::headline::REGEX_HEADLINE_SHORT;
    use regex::Regex;

    #[test]
    fn essentials() {
        let input = "1234567890\nЗдравствуйте";
        let mut cursor = Cursor::new(&input, 0);
        assert_eq!('1', cursor.get_next_char().unwrap());
        assert_eq!(1, cursor.pos());
        assert_eq!('2', cursor.get_next_char().unwrap());
        assert_eq!(2, cursor.pos());
        assert_eq!(11, cursor.next::<LinesMetric>().unwrap());
        assert!(cursor.is_boundary::<LinesMetric>());
        assert_eq!('З', cursor.get_next_char().unwrap());
        assert_eq!(13, cursor.pos());
        cursor.set(12);
        assert!(!cursor.is_boundary::<BaseMetric>());
    }

    #[test]
    fn count_between() {
        let input = "\n \n \n";
        let cursor = Cursor::new(&input, 0);
        assert_eq!(3, cursor.count_between::<LinesMetric>(0, 5));
    }

    #[test]
    fn looking_at_headline() {
        let rope = "Some text\n**** headline\n";
        let mut cursor = Cursor::new(&rope, 0);
        assert!(cursor.looking_at(&*REGEX_HEADLINE_SHORT).is_none());

        cursor.set(4);
        assert!(cursor.looking_at(&*REGEX_HEADLINE_SHORT).is_none());
        assert_eq!(4, cursor.pos());

        cursor.set(15);
        assert!(cursor.looking_at(&*REGEX_HEADLINE_SHORT).is_none());

        cursor.set(10);

        let m = cursor.looking_at(&*REGEX_HEADLINE_SHORT).unwrap();
        assert_eq!(0, m.start());
        assert_eq!(5, m.end());
        assert_eq!("**** ", m.as_str());
        assert_eq!(10, cursor.pos());
    }

    #[test]
    fn looking_at_empty_line_re() {
        let text = "First line\n   \n\nFourth line";
        let mut cursor = Cursor::new(&text, 0);

        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_none());
        cursor.goto_next_line();
        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_some());
        cursor.goto_next_line();
        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_some());
        cursor.goto_next_line();
        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_none());
    }

    #[test]
    fn on_headline() {
        let rope = "Some text\n**** headline\n";
        let mut cursor = Cursor::new(&rope, 0);

        assert!(!cursor.on_headline());

        cursor.set(4);
        assert!(!cursor.on_headline());
        assert_eq!(4, cursor.pos());

        cursor.set(15);
        assert!(cursor.on_headline());

        cursor.set(10);
        assert!(cursor.on_headline());
        assert_eq!(10, cursor.pos());
    }

    #[test]
    fn next_headline() {
        let string = "Some text\n**** headline\n";
        let mut cursor = Cursor::new(&string, 0);

        assert_eq!(Some(10), cursor.next_headline());
        assert_eq!(10, cursor.pos());

        let string2 = "* First\n** Second\n";
        cursor = Cursor::new(&string2, 0);
        assert_eq!(Some(8), cursor.next_headline());
        assert_eq!(8, cursor.pos());
    }

    #[test]
    fn skip_whitespaces() {
        let rope = " \n\t\rorg-mode ";
        let mut cursor = Cursor::new(&rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_next_char().unwrap(), 'o');

        let rope2 = "no_whitespace_for_you!";
        cursor = Cursor::new(&rope2, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_next_char().unwrap(), 'n');

        // Skipping all the remaining whitespace results in invalid cursor at the end of the rope
        let rope3 = " ";
        cursor = Cursor::new(&rope3, 0);
        cursor.skip_whitespace();
        assert_eq!(None, cursor.get_next_char());
    }

    #[test]
    fn skip_whitespaces_backwards() {
        let rope = " org-mode \n\t\r";
        let mut cursor = Cursor::new(&rope, rope.len() - 1);
        cursor.skip_whitespace_backwards();
        assert_eq!(cursor.get_next_char().unwrap(), 'e');

        let rope2 = "no_whitespace_for_you!";
        cursor = Cursor::new(&rope2, rope2.len() - 1);
        cursor.skip_whitespace_backwards();
        assert_eq!(cursor.get_next_char().unwrap(), '!');

        // Skipping all the remaining whitespace results in invalid cursor at the beginning of the rope
        let rope3 = " ";
        cursor = Cursor::new(&rope3, 1);
        cursor.skip_whitespace_backwards();
        assert_eq!(None, cursor.get_prev_char());
    }

    #[test]
    fn skip_whitespace_odd_count() {
        let rope = "   xyz";
        let mut cursor = Cursor::new(&rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_next_char().unwrap(), 'x');
    }

    #[test]
    fn skip_whitespace_backwards_odd_count() {
        let rope = "xyz   ";
        let mut cursor = Cursor::new(&rope, rope.len() - 1);
        cursor.skip_whitespace_backwards();
        assert_eq!(cursor.get_next_char().unwrap(), 'z');
    }

    #[test]
    fn line_begin() {
        let rope = "First line\nSecond line\r\nThird line";
        let mut cursor = Cursor::new(&rope, 13);
        assert_eq!(cursor.goto_line_begin(), 11);
        assert_eq!(cursor.goto_line_begin(), 11);
        assert_eq!(cursor.goto_line_begin(), 11);
        cursor.set(26);
        assert_eq!(cursor.goto_line_begin(), 24);
        assert!(cursor.is_bol());
        assert_eq!(cursor.get_next_char().unwrap(), 'T');
        assert_eq!(cursor.goto_line_begin(), 24);
        assert_eq!(cursor.get_next_char().unwrap(), 'T');
        cursor.set(3);
        assert_eq!(cursor.goto_line_begin(), 0);
        assert_eq!(cursor.get_next_char().unwrap(), 'F');
    }

    #[test]
    fn prev_line() {
        let rope = "First line\nSecond line\r\nThird line\nFour";
        let mut cursor = Cursor::new(&rope, rope.len());

        assert_eq!(cursor.goto_prev_line(), 24);
        assert_eq!(cursor.get_next_char().unwrap(), 'T');

        assert_eq!(cursor.goto_prev_line(), 11);
        assert_eq!(cursor.get_next_char().unwrap(), 'S');

        assert_eq!(cursor.goto_prev_line(), 0);
        assert_eq!(cursor.get_next_char().unwrap(), 'F');
    }

    #[test]
    fn line_begin_pos() {
        let rope = "One\nTwo\nThi\nFo4\nFiv\nSix\n7en";
        let mut cursor = Cursor::new(&rope, 13);

        assert_eq!(cursor.line_beginning_position(None), 12);
        assert_eq!(cursor.line_beginning_position(Some(1)), 12);
        assert_eq!(cursor.line_beginning_position(Some(2)), 16);
        assert_eq!(cursor.line_beginning_position(Some(3)), 20);

        assert_eq!(cursor.line_beginning_position(Some(0)), 8);
        assert_eq!(cursor.line_beginning_position(Some(-1)), 4);
        assert_eq!(cursor.line_beginning_position(Some(-2)), 0);
    }

    #[test]
    fn line_end_pos() {
        let text = "One\nTwo\nThi\nFo4\nFiv\nSix\n7en";
        let mut cursor = Cursor::new(&text, 13);

        assert_eq!(27, text.len());
        // Moving forward
        assert_eq!(cursor.line_end_position(None), 15);
        assert_eq!(cursor.line_end_position(Some(1)), 15);
        assert_eq!(cursor.line_end_position(Some(2)), 19);
        assert_eq!(cursor.line_end_position(Some(3)), 23);
        assert_eq!(cursor.line_end_position(Some(4)), 26);

        //Moving backward
        assert_eq!(cursor.line_end_position(Some(0)), 11);
        assert_eq!(cursor.line_end_position(Some(-1)), 7);
        assert_eq!(cursor.line_end_position(Some(-2)), 3);
        assert_eq!(cursor.line_end_position(Some(-3)), 3);
    }

    #[test]
    fn is_bol() {
        let rope = "One\nTwo\nThi\nFo4\nFiv\nSix\n7en";
        let mut cursor = Cursor::new(&rope, 0);
        assert!(cursor.is_bol());
        cursor.set(2);
        assert!(!cursor.is_bol());
        cursor.set(4);
        assert!(cursor.is_bol());
        cursor.set(rope.len());
        assert!(!cursor.is_bol());

        cursor.prev::<LinesMetric>();
        assert!(cursor.is_bol());
        cursor.goto_prev_line();
        assert!(cursor.is_bol());
        cursor.goto_next_line();
        assert!(cursor.is_bol());
    }

    #[test]
    fn search_forward() {
        let str = "onetwothreefouronetwothreeonetwothreeonetwothreefouroneabababa";
        let mut cursor = Cursor::new(&str, 0);
        assert_eq!(cursor.search_forward("one", None, Some(2)), Some(18));
        assert_eq!(cursor.search_forward("one", None, None), Some(29));
        cursor.set(0);
        assert_eq!(cursor.search_forward("threeone", Some(10), None), None); // there is no match before 10th pos
        assert_eq!(cursor.search_forward("threeone", Some(100), Some(10)), None); // there is not a 10th match so return None
        assert_eq!(cursor.search_forward("two", None, Some(4)), Some(43));
        assert_eq!(cursor.pos(), 43);
        assert_eq!(cursor.search_forward("aba", Some(10), None), None); // bound is before current pos
        assert_eq!(cursor.pos(), 43);
        assert_eq!(cursor.search_forward("aba", Some(10000), Some(2)), Some(62));
        cursor.set(0);
        assert_eq!(cursor.search_forward("aba", Some(10000), Some(6)), None);
    }

    #[test]
    fn skip_chars_forward() {
        let str = "  k\t **hello";
        let mut cursor = Cursor::new(&str, 0);
        assert_eq!(cursor.skip_chars_forward(" ", None), 2);
        assert_eq!(cursor.pos(), 2);
        assert_eq!(cursor.skip_chars_forward(" k\t", None), 3);
        cursor.set(0);
        assert_eq!(cursor.skip_chars_forward("* k\t", Some(2)), 3);
    }

    #[test]
    fn re_search_forward() {
        let text = "One\nTwo\nThi\nFo4\nFiv\nSix\n7en";
        let mut cursor = Cursor::new(&text, 0);

        let re = Regex::new(r"\d").unwrap();
        assert_eq!(Some(15), cursor.re_search_forward(&re, None));
        assert_eq!(15, cursor.pos());
        assert_eq!(None, cursor.re_search_forward(&re, Some(10)));
        assert_eq!(15, cursor.pos());
        assert_eq!(Some(25), cursor.re_search_forward(&re, Some(25)));
        assert_eq!(None, cursor.re_search_forward(&re, Some(24)));
        assert_eq!(25, cursor.pos());
    }
}
