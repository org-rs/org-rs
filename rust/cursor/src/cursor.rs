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

use crate::{Cursor, Interval};

use crate::lexeme::Lexeme;
use crate::metrics::Metric;


impl<'a> Cursor<'a> {
    pub fn new(data: &'a str, pos: usize) -> Cursor<'a> {
        Cursor { data, pos }
    }

    // total length of the underlying data
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn eof(&mut self) {
        self.pos = self.data.len();
    }

    pub fn bof(&mut self) {
        self.pos = 0;
    }

    pub fn inc(&mut self, inc: usize) {
        self.pos = self.pos + inc;
    }

    pub fn dec(&mut self, dec: usize) {
        if dec > self.pos {
            self.pos = 0;
        } else {
            self.pos = self.pos - dec;
        }
    }
}

pub trait MetricCursor {
    fn is_boundary<M: Metric>(&self) -> bool;
    fn mnext<M: Metric>(&mut self) -> Option<usize>;
    fn mprev<M: Metric>(&mut self) -> Option<usize>;
    fn at_or_mnext<M: Metric>(&mut self) -> Option<usize>;
    fn at_or_mprev<M: Metric>(&mut self) -> Option<usize>;
}

impl<'a> MetricCursor for Cursor<'a> {
    fn is_boundary<M: Metric>(&self) -> bool {
        M::is_boundary(self.data, self.pos)
    }

    fn mnext<M: Metric>(&mut self) -> Option<usize> {
        if let Some(l) = M::next(self.data, self.pos) {
            self.pos = l;
            Some(l)
        } else {
            None
        }
    }

    fn mprev<M: Metric>(&mut self) -> Option<usize> {
        if let Some(offset) = M::prev(self.data, self.pos) {
            self.pos = offset;
            Some(offset)
        } else {
            None
        }
    }

    fn at_or_mnext<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.mnext::<M>()
        }
    }

    fn at_or_mprev<M: Metric>(&mut self) -> Option<usize> {
        if self.is_boundary::<M>() {
            Some(self.pos())
        } else {
            self.mprev::<M>()
        }
    }
}

pub trait LexemeCursor {
    fn is_on<L: Lexeme>(&self) -> bool;
    fn lprev<L: Lexeme>(&mut self) -> Option<Interval>;
    fn lnext<L: Lexeme>(&mut self) -> Option<Interval>;
    fn get_lprev<L: Lexeme>(&mut self) -> Option<L::Item>;
    fn get_lnext<L: Lexeme>(&mut self) -> Option<L::Item>;
}

impl<'a> LexemeCursor for Cursor<'a> {
    fn is_on<L: Lexeme>(&self) -> bool {
        L::is_on(self.data, self.pos)
    }

    fn lprev<L: Lexeme>(&mut self) -> Option<Interval> {
        match L::find_prev(self.data, self.pos) {
            None => None,
            Some(i) => {
                self.set(i.start);
                Some(i)
            }
        }
    }

    fn lnext<L: Lexeme>(&mut self) -> Option<Interval> {
        match L::find_next(self.data, self.pos) {
            None => None,
            Some(i) => {
                self.set(i.start);
                Some(i)
            }
        }
    }

    fn get_lprev<L: Lexeme>(&mut self) -> Option<L::Item> {
        match L::get_prev(self.data, self.pos) {
            None => None,
            Some(item) => {
                self.set(item.address.start);
                Some(item.value)
            }
        }
    }

    fn get_lnext<L: Lexeme>(&mut self) -> Option<L::Item> {
        match L::get_next(self.data, self.pos) {
            None => None,
            Some(item) => {
                self.set(item.address.start);
                Some(item.value)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::Cursor;
    use super::REGEX_EMPTY_LINE;
    use crate::Char;
    use crate::Line;

    use crate::cursor::{LexemeCursor, MetricCursor};
    use crate::emacs::EmacsCursor;
    use crate::search::SearchCursor;
    //use regex::Match;
    use regex::Regex;

    #[test]
    fn essentials() {
        let input = "1234567890\nЗдравствуйте";
        let mut cursor = Cursor::new(&input, 0);
        assert_eq!('1', cursor.get_lnext::<Char>().unwrap());
        assert_eq!(1, cursor.pos());
        assert_eq!('2', cursor.get_lnext::<Char>().unwrap());
        assert_eq!(2, cursor.pos());
        assert_eq!(11, cursor.mnext::<Line>().unwrap());
        assert!(cursor.is_boundary::<Line>());
        assert_eq!('З', cursor.get_lnext::<Char>().unwrap());
        assert_eq!(13, cursor.pos());
        cursor.set(12);
        assert!(!cursor.is_boundary::<Char>());
    }

    #[test]
    fn looking_at_empty_line_re() {
        let text = "First line\n   \n\nFourth line";
        let mut cursor = Cursor::new(&text, 0);

        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_none());
        cursor.mnext::<Line>();
        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_some());
        cursor.mnext::<Line>();
        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_some());
        cursor.mnext::<Line>();
        assert!(cursor.looking_at(&*REGEX_EMPTY_LINE).is_none());
    }

    #[test]
    fn skip_whitespaces() {
        let rope = " \n\t\rorg-mode ";
        let mut cursor = Cursor::new(&rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'o');

        let rope2 = "no_whitespace_for_you!";
        cursor = Cursor::new(&rope2, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'n');

        // Skipping all the remaining whitespace results in invalid cursor at the end of the rope
        let rope3 = " ";
        cursor = Cursor::new(&rope3, 0);
        cursor.skip_whitespace();
        assert_eq!(None, cursor.lnext::<Char>());
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
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'T');
        assert_eq!(cursor.goto_line_begin(), 24);
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'T');
        cursor.set(3);
        assert_eq!(cursor.goto_line_begin(), 0);
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'F');
    }

    #[test]
    fn prev_line() {
        let rope = "First line\nSecond line\r\nThird line\nFour";
        let mut cursor = Cursor::new(&rope, rope.len());

        assert_eq!(cursor.get_lprev::<Line>(), Some("Third line\n".to_owned()));
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'T');

        assert_eq!(cursor.lprev::<Line>().unwrap().start, 11);
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'S');

        assert_eq!(cursor.lprev::<Line>().unwrap().start, 0);
        assert_eq!(cursor.get_lnext::<Char>().unwrap(), 'F');
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

        cursor.mprev::<Line>();
        assert!(cursor.is_bol());
        cursor.mprev::<Line>();
        assert!(cursor.is_bol());
        cursor.mnext::<Line>();
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
    fn skip_chars_backward() {
        let text = "This is some text 123 \t\n\r";
        let mut cursor = Cursor::new(&text, text.len());
        assert_eq!(8, cursor.skip_chars_backward(" \t\n\r123", None));
        assert_eq!(17, cursor.pos());
        assert_eq!(' ', cursor.get_lnext::<Char>().unwrap());

        cursor.set(text.len());
        assert_eq!(1, cursor.skip_chars_backward(" \t\n\r", Some(24)));
        assert_eq!('\r', cursor.get_lnext::<Char>().unwrap());

        let txt2 = "Text";
        cursor = Cursor::new(&txt2, txt2.len());
        assert_eq!(0, cursor.skip_chars_backward("", None));
    }

    #[test]
    fn re_search_forward() {
        let text = "One\nTwo\nThi\nFo4\nFiv\nSix\n7en";
        let mut cursor = Cursor::new(&text, 0);

        let re = Regex::new(r"\d").unwrap();
        assert_eq!(14, cursor.re_search_forward(&re, None).unwrap().start);
        assert_eq!(15, cursor.pos());
        assert_eq!(None, cursor.re_search_forward(&re, Some(10)));
        assert_eq!(15, cursor.pos());
        assert_eq!(24, cursor.re_search_forward(&re, Some(25)).unwrap().start);
        assert_eq!(25, cursor.pos());
        assert_eq!(None, cursor.re_search_forward(&re, Some(24)));
        assert_eq!(25, cursor.pos());
    }
}
