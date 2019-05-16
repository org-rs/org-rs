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

use crate::headline::REGEX_HEADLINE_MULTILINE;
use crate::headline::REGEX_HEADLINE_SHORT;
use regex::Regex;

use memchr::{memchr, memrchr};

pub trait Metric {
    fn is_boundary(s: &str, offset: usize) -> bool;
    fn prev(s: &str, offset: usize) -> Option<usize>;
    fn next(s: &str, offset: usize) -> Option<usize>;
}

pub struct BaseMetric(());
pub struct LinesMetric(());

impl Metric for BaseMetric {
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

impl Metric for LinesMetric {
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

pub struct Cursor<'a> {
    data: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a str, pos: usize) -> Cursor<'a> {
        Cursor { data, pos }
    }

    pub fn set(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Get next codepoint after cursor position, and advance cursor.
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
    pub fn get_prev_char(&mut self) -> Option<char> {
        if let Some(offset) = self.prev::<BaseMetric>() {
            self.pos = offset;
            self.data[offset..].chars().next()
        } else {
            None
        }
    }

    pub fn next<M: Metric>(&mut self) -> Option<usize> {
        if let Some(offset) = M::next(self.data, self.pos) {
            self.pos = offset;
            Some(offset)
        } else {
            None
        }
    }

    pub fn is_boundary<M: Metric>(&self) -> bool {
        M::is_boundary(self.data, self.pos)
    }

    pub fn prev<M: Metric>(&mut self) -> Option<usize> {
        if let Some(offset) = M::prev(self.data, self.pos) {
            self.pos = offset;
            Some(offset)
        } else {
            None
        }
    }

    pub fn at_or_next<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.next::<M>()
        }
    }

    pub fn at_or_prev<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.prev::<M>()
        }
    }
}

/// Handy things for cursor
pub trait CursorHelper {
    /// Skip over space, tabs and newline characters
    /// Cursor position is set before next non-whitespace char
    fn skip_whitespace(&mut self) -> usize;

    /// Moves cursor to the beginning of the current line.
    /// Acts like "Home" button
    /// If cursor is already at the beginning of the line - nothing happens
    /// Returns the position of the cursor
    fn goto_line_begin(&mut self) -> usize;

    /// Moves cursor to the beginning of the next line. If there is no next line
    /// cursor position is set to len() of the rope
    fn goto_next_line(&mut self) -> usize;

    /// Moves cursor to the beginning of the previous line.
    /// If there is no previous line then cursor position
    /// is set the beginning of the rope - 0
    fn goto_prev_line(&mut self) -> usize;

    /// corresponds to `line-beginning-position` in elisp
    /// Return the character position of the first character on the current line.
    /// If N is none then acts as `goto_line_begin`
    /// Otherwise moves forward N - 1 lines first.
    /// with N < 1 cursor will move to previous lines
    ///
    /// This function does not move the cursor (does save-excursion)
    fn line_beginning_position(&mut self, n: Option<i32>) -> usize;

    fn char_after(&mut self, offset: usize) -> Option<char>;

    /// Checks if current line matches a given regex
    /// This function determines whether the text in
    /// the current buffer directly following cursor matches
    /// the regular expression regexp.
    /// “Directly following” means precisely that:
    /// the search is “anchored” and it can succeed only
    /// starting with the first character following point.
    /// The result is true if so, false otherwise.
    /// This function does not move cursor
    fn looking_at(&self, r: &Regex) -> bool;

    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    /// If next headline is found returns it's start position
    fn next_headline(&mut self) -> Option<(usize)>;

    /// Return true if cursor is on a headline.
    /// corresponds to `org-at-heading-p`
    fn on_headline(&mut self) -> bool;

    fn is_bol(&self) -> bool;
}

// Implementation for xi-rope
impl<'a> CursorHelper for Cursor<'a> {
    fn skip_whitespace(&mut self) -> usize {
        while let Some(c) = self.get_next_char() {
            if !(c.is_whitespace()) {
                self.get_prev_char();
                break;
            } else {
                self.get_next_char();
            }
        }
        self.pos()
    }

    fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 && self.at_or_prev::<LinesMetric>().is_none() {
            self.set(0);
        }
        self.pos()
    }

    fn goto_next_line(&mut self) -> usize {
        let res = self.next::<LinesMetric>();
        match res {
            None => {
                self.set(self.data.len());
                self.data.len()
            }
            Some(x) => x,
        }
    }

    fn goto_prev_line(&mut self) -> usize {
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

    fn line_beginning_position(&mut self, n: Option<i32>) -> usize {
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
                        for p in 0..(x - 1).abs() {
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
        return result;
    }

    fn char_after(&mut self, offset: usize) -> Option<char> {
        let pos = self.pos();
        self.set(offset);
        let result = self.get_next_char();
        self.set(pos);
        return result;
    }

    fn looking_at(&self, re: &Regex) -> bool {
        re.find(&self.data[self.pos..]).is_some()
    }

    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    /// If next headline is found returns it's start position
    fn next_headline(&mut self) -> Option<(usize)> {
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
    fn on_headline(&mut self) -> bool {
        let pos = self.pos();
        self.goto_line_begin();
        let result = self.looking_at(&*REGEX_HEADLINE_SHORT);
        self.set(pos);
        return result;
    }

    fn is_bol(&self) -> bool {
        if self.pos == 0 {
            true
        } else {
            LinesMetric::is_boundary(self.data, self.pos)
        }
    }
}

/// Given the inital byte of a UTF-8 codepoint, returns the number of
/// bytes required to represent the codepoint.
/// RFC reference : https://tools.ietf.org/html/rfc3629#section-4
pub fn len_utf8_from_first_byte(b: u8) -> usize {
    match b {
        b if b < 0x80 => 1,
        b if b < 0xe0 => 2,
        b if b < 0xf0 => 3,
        _ => 4,
    }
}

/// Checks if a regular expression can match multiple lines.
pub fn is_multiline_regex(regex: &str) -> bool {
    // regex characters that match line breaks
    // todo: currently multiline mode is ignored
    let multiline_indicators = vec![r"\n", r"\r", r"[[:space:]]"];

    multiline_indicators.iter().any(|&i| regex.contains(i))
}

mod test {
    use std::str::FromStr;

    use super::Cursor;
    use super::CursorHelper;
    use super::LinesMetric;
    use super::Metric;

    use crate::data::Syntax;
    use crate::headline::REGEX_HEADLINE_SHORT;
    use crate::parser::Parser;

    use crate::cursor::BaseMetric;

    #[test]
    fn essetials() {
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
    fn looking_at() {
        let rope = "Some text\n**** headline\n";
        let mut cursor = Cursor::new(&rope, 0);
        assert!(!cursor.looking_at(&*REGEX_HEADLINE_SHORT));

        cursor.set(4);
        assert!(!cursor.looking_at(&*REGEX_HEADLINE_SHORT));
        assert_eq!(4, cursor.pos());

        cursor.set(15);
        assert!(!cursor.looking_at(&*REGEX_HEADLINE_SHORT));

        cursor.set(10);

        assert!(cursor.looking_at(&*REGEX_HEADLINE_SHORT));
        assert_eq!(10, cursor.pos());
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
}
