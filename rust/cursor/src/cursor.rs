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


}
