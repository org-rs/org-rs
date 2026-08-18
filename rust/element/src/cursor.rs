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

pub mod metrics;
pub use metrics::{BaseMetric, LinesMetric, Metric};

mod headline;

use memchr::memchr;
use regex::{Captures, Match, Regex};

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

/// Checks if a regular expression can match multiple lines.
#[inline]
pub(crate) fn is_multiline_regex(regex: &str) -> bool {
    const MULTILINE_INDICATORS: &[&str] = &[r"\n", r"\r", r"[[:space:]]"];
    MULTILINE_INDICATORS.iter().any(|i| regex.contains(i))
}

lazy_static! {
    /// Empty-line pattern, retained here for the pre-cutover parser, which
    /// matches empty lines through the cursor. Superseded when `affiliated.rs`
    /// is ported to the new parser.
    pub static ref REGEX_EMPTY_LINE: CachedRegex =
        CachedRegex::new(Regex::new(r"^[ \t]*$").unwrap());
}

pub struct Cursor<'a> {
    pub data: &'a str,
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    #[inline]
    pub fn new(data: &'a str, pos: usize) -> Cursor<'a> {
        Cursor { data, pos }
    }

    #[inline]
    pub fn set(&mut self, pos: usize) {
        if !self.data.is_char_boundary(pos) {
            // Adjust backward to the nearest valid char boundary.
            // This prevents panics when an element's location.end lands
            // inside a multi-byte character (e.g. with non-ASCII text).
            let mut p = pos.min(self.data.len());
            while p > 0 && !self.data.is_char_boundary(p) {
                p -= 1;
            }
            self.pos = p;
        } else {
            self.pos = pos;
        }
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

    /// Peek the character at `offset` without moving the cursor. Retained for
    /// the pre-cutover parser; the new parser reads bytes directly.
    #[inline]
    pub fn char_after(&mut self, offset: usize) -> Option<char> {
        let pos = self.pos();
        self.set(offset);
        let result = self.get_next_char();
        self.set(pos);
        result
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
    pub fn at_or_prev<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.prev::<M>()
        }
    }

    /// Skip over space, tabs and newline characters.
    /// Cursor position is set before next non-whitespace char.
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

    /// Moves cursor to the beginning of the current line.
    /// Acts like "Home" button.
    /// If cursor is already at the beginning of the line - nothing happens.
    /// Returns the position of the cursor.
    #[inline]
    pub fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 && self.at_or_prev::<LinesMetric>().is_none() {
            self.set(0);
        }
        self.pos()
    }

    /// Moves cursor to the beginning of the next line. If there is no next line
    /// cursor position is set to len() of the input.
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
    /// is set the beginning of the rope - 0.
    #[inline]
    pub fn goto_prev_line(&mut self) -> usize {
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
    /// If N is none then acts as `goto_line_begin`.
    /// Otherwise moves forward N - 1 lines first.
    /// With N < 1 cursor will move to previous lines.
    ///
    /// Corresponds to `line-beginning-position` in elisp.
    /// This function does not move the cursor (does save-excursion).
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
    /// Corresponds to `line-end-position` in elisp.
    /// This function does not move the cursor (does save-excursion).
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

    /// Checks if current line matches a given regex.
    ///
    /// "Directly following" means the search is anchored: it can succeed
    /// only starting with the first character following point.
    /// Does not move the cursor.
    /// Use `capturing_at` if you need capture groups.
    #[inline]
    pub fn looking_at(&self, re: &CachedRegex) -> Option<Match<'a>> {
        let end = if !re.multiline() {
            LinesMetric::next(self.data, self.pos)
                .map(|p| p - 1)
                .unwrap_or_else(|| self.data.len())
        } else {
            self.data.len()
        };
        re.find(&self.data[self.pos..end])
            .filter(|m| m.start() == 0)
    }

    /// Acts exactly as `looking_at` but returns Captures.
    /// This is slower than simple regex search so if you don't need
    /// capture groups use `looking_at` for better performance.
    #[inline]
    pub fn capturing_at(&self, re: &CachedRegex) -> Option<Captures<'a>> {
        let end = if !re.multiline() {
            LinesMetric::next(self.data, self.pos)
                .map(|p| p - 1)
                .unwrap_or_else(|| self.data.len())
        } else {
            self.data.len()
        };
        re.captures(&self.data[self.pos..end])
    }

    /// Returns `true` if every byte on the current line (up to but not
    /// including the newline) is a space or tab. Does not move the cursor.
    #[inline]
    pub fn is_line_blank(&self) -> bool {
        let bytes = self.data.as_bytes();
        let end = memchr(b'\n', &bytes[self.pos..])
            .map(|i| self.pos + i)
            .unwrap_or(bytes.len());
        bytes[self.pos..end]
            .iter()
            .all(|&b| b == b' ' || b == b'\t')
    }

    #[inline]
    pub fn is_bol(&self) -> bool {
        if self.pos == 0 {
            true
        } else {
            LinesMetric::is_boundary(self.data, self.pos)
        }
    }
}

#[cfg(test)]
mod test {
    use super::{BaseMetric, CachedRegex, Cursor, LinesMetric};
    use regex::Regex;

    fn headline_re() -> CachedRegex {
        CachedRegex::new(Regex::new(r"^\*+\s").unwrap())
    }

    #[test]
    fn essentials() {
        let input = "1234567890\nЗдравствуйте";
        let mut cursor = Cursor::new(input, 0);
        assert_eq!('1', cursor.get_next_char().unwrap());
        assert_eq!(1, cursor.pos());
        assert_eq!('2', cursor.get_next_char().unwrap());
        assert_eq!(2, cursor.pos());
        assert_eq!(11, cursor.next::<LinesMetric>().unwrap());
        assert!(cursor.is_boundary::<LinesMetric>());
        assert_eq!('З', cursor.get_next_char().unwrap());
        assert_eq!(13, cursor.pos());
        cursor.set(12);
        assert_eq!(11, cursor.pos());
        assert!(cursor.is_boundary::<BaseMetric>());
    }

    #[test]
    fn looking_at_headline() {
        let re = headline_re();
        let rope = "Some text\n**** headline\n";
        let mut cursor = Cursor::new(rope, 0);
        assert!(cursor.looking_at(&re).is_none());

        cursor.set(4);
        assert!(cursor.looking_at(&re).is_none());
        assert_eq!(4, cursor.pos());

        cursor.set(15);
        assert!(cursor.looking_at(&re).is_none());

        cursor.set(10);
        let m = cursor.looking_at(&re).unwrap();
        assert_eq!(0, m.start());
        assert_eq!(5, m.end());
        assert_eq!("**** ", m.as_str());
        assert_eq!(10, cursor.pos());
    }

    #[test]
    fn is_blank_line() {
        let text = "First line\n   \n\nFourth line";
        let cursor = Cursor::new(text, 0);
        assert!(!cursor.is_line_blank());

        let mut c = Cursor::new(text, 0);
        c.goto_next_line();
        assert!(c.is_line_blank());

        c.goto_next_line();
        assert!(c.is_line_blank());

        c.goto_next_line();
        assert!(!c.is_line_blank());
    }

    #[test]
    fn on_headline() {
        let rope = "Some text\n**** headline\n";
        let mut cursor = Cursor::new(rope, 0);

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
        let mut cursor = Cursor::new(string, 0);
        assert_eq!(Some(10), cursor.next_headline());
        assert_eq!(10, cursor.pos());

        let string2 = "* First\n** Second\n";
        cursor = Cursor::new(string2, 0);
        assert_eq!(Some(8), cursor.next_headline());
        assert_eq!(8, cursor.pos());
    }

    #[test]
    fn skip_whitespaces() {
        let rope = " \n\t\rorg-mode ";
        let mut cursor = Cursor::new(rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_next_char().unwrap(), 'o');

        let rope2 = "no_whitespace_for_you!";
        cursor = Cursor::new(rope2, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_next_char().unwrap(), 'n');

        let rope3 = " ";
        cursor = Cursor::new(rope3, 0);
        cursor.skip_whitespace();
        assert_eq!(None, cursor.get_next_char());
    }

    #[test]
    fn skip_whitespace_odd_count() {
        let rope = "   xyz";
        let mut cursor = Cursor::new(rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_next_char().unwrap(), 'x');
    }

    #[test]
    fn line_begin() {
        let rope = "First line\nSecond line\r\nThird line";
        let mut cursor = Cursor::new(rope, 13);
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
        let mut cursor = Cursor::new(rope, rope.len());
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
        let mut cursor = Cursor::new(rope, 13);
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
        let mut cursor = Cursor::new(text, 13);
        assert_eq!(27, text.len());
        assert_eq!(cursor.line_end_position(None), 15);
        assert_eq!(cursor.line_end_position(Some(1)), 15);
        assert_eq!(cursor.line_end_position(Some(2)), 19);
        assert_eq!(cursor.line_end_position(Some(3)), 23);
        assert_eq!(cursor.line_end_position(Some(4)), 26);
        assert_eq!(cursor.line_end_position(Some(0)), 11);
        assert_eq!(cursor.line_end_position(Some(-1)), 7);
        assert_eq!(cursor.line_end_position(Some(-2)), 3);
        assert_eq!(cursor.line_end_position(Some(-3)), 3);
    }

    #[test]
    fn is_bol() {
        let rope = "One\nTwo\nThi\nFo4\nFiv\nSix\n7en";
        let mut cursor = Cursor::new(rope, 0);
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
}
