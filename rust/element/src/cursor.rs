use xi_rope::{Cursor, RopeInfo, LinesMetric};
use regex::Regex;
use xi_rope::find::CaseMatching::CaseInsensitive;
use xi_rope::find::find;
use crate::headline::REGEX_HEADLINE_SHORT;


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

    /// Checks if current line of the cursor is a headline
    /// In emacs defined as org-at-heading-p which is a proxy to
    /// outline-on-heading-p at outline.el
    fn looking_at(&mut self, r: &Regex) -> bool;


    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    /// If next headline is found returns it's start position
    fn next_headline(&mut self) -> Option<(usize)> ;
}

impl<'a> CursorHelper for Cursor<'a, RopeInfo> {
    fn skip_whitespace(&mut self) -> usize {
        while let Some(c) = self.next_codepoint() {
            if !(c.is_whitespace()) {
                self.prev_codepoint();
                break;
            } else {
                self.next_codepoint();
            }
        }
        self.pos()
    }

    fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 {
            if self.at_or_prev::<LinesMetric>().is_none() {
                self.set(0);
            }
        }
        self.pos()
    }

    fn goto_next_line(&mut self) -> usize {
        let res = self.next::<LinesMetric>();
        match res {
            None => {
                self.set(self.root().len());
                self.root().len()
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
                    for p in 0..x - 1 {
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
        let result = self.next_codepoint();
        self.set(pos);
        return result;
    }

    fn looking_at(&mut self, r: &Regex) -> bool {
        let pos = self.pos();
        let beg = self.goto_line_begin();
        self.set(pos);
        let mut raw_lines = self.root().lines_raw(beg..self.root().len());
        match raw_lines.next() {
            Some(line) => r.is_match(&line),
            None => false,
        }
    }


    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    /// If next headline is found returns it's start position
    fn next_headline(&mut self) -> Option<(usize)> {
        let pos = self.pos();
        let mut raw_lines = self
            .root()
            .lines_raw(self.pos()..self.root().len());
        // make sure we don't match current headline
        raw_lines.next();
        self.next::<LinesMetric>();

        // TODO consider using FULL headline regex?
        let search = find(
            self,
            &mut raw_lines,
            CaseInsensitive,
            &*REGEX_HEADLINE_SHORT.as_str(),
            Some(&*REGEX_HEADLINE_SHORT),
        );
        match search {
            None => {
                self.set(pos);
                None
            }
            Some(begin) => {
                self.set(begin);
                Some(begin)
            }
        }
    }
}

mod test {
    use core::borrow::Borrow;
    use std::str::FromStr;

    use xi_rope::LinesMetric;
    use xi_rope::find::find;
    use xi_rope::find::CaseMatching::CaseInsensitive;
    use xi_rope::{Cursor, Rope};

    use crate::data::Syntax;
    use crate::parser::Parser;
    use crate::headline::REGEX_HEADLINE_SHORT;

    use super::CursorHelper;

    #[test]
    fn looking_at() {
        let rope = Rope::from_str("Some text\n**** headline\n").unwrap();
        let mut cursor = Cursor::new(&rope, 0);

        assert!(!cursor.looking_at(&*REGEX_HEADLINE_SHORT));

        cursor.set(4);
        assert!(!cursor.looking_at(&*REGEX_HEADLINE_SHORT));
        assert_eq!(4, cursor.pos());

        cursor.set(15);
        assert!(cursor.looking_at(&*REGEX_HEADLINE_SHORT));
        assert_eq!(15, cursor.pos());
    }

    #[test]
    fn next_headline() {
        let rope = Rope::from_str("Some text\n**** headline\n").unwrap();
        let mut cursor = Cursor::new(&rope, 0);

        assert_eq!(Some(10), cursor.next_headline());
        assert_eq!(10, cursor.pos());

        let rope = Rope::from_str("* First\n** Second\n").unwrap();
        cursor = Cursor::new(&rope, 0);
        assert_eq!(Some(8), cursor.next_headline());
        assert_eq!(8, cursor.pos());
    }

    #[test]
    fn skip_whitespaces() {
        let rope = Rope::from_str(" \n\t\rorg-mode ").unwrap();
        let mut cursor = Cursor::new(&rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.next_codepoint().unwrap(), 'o');

        let rope2 = Rope::from_str("no_whitespace_for_you!").unwrap();
        cursor = Cursor::new(&rope2, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.next_codepoint().unwrap(), 'n');

        // Skipping all the remaining whitespace results in invalid cursor at the end of the rope
        let rope3 = Rope::from_str(" ").unwrap();
        cursor = Cursor::new(&rope3, 0);
        cursor.skip_whitespace();
        assert_eq!(None, cursor.next_codepoint());
    }

    #[test]
    fn line_begin() {
        let rope = Rope::from_str("First line\nSecond line\r\nThird line").unwrap();
        let mut cursor = Cursor::new(&rope, 13);
        assert_eq!(cursor.goto_line_begin(), 11);
        assert_eq!(cursor.peek_next_codepoint().unwrap(), 'S');
        assert_eq!(cursor.goto_line_begin(), 11);
        assert_eq!(cursor.goto_line_begin(), 11);
        assert_eq!(cursor.peek_next_codepoint().unwrap(), 'S');
        cursor.set(26);
        assert_eq!(cursor.goto_line_begin(), 24);
        assert_eq!(cursor.peek_next_codepoint().unwrap(), 'T');
        assert_eq!(cursor.next_codepoint().unwrap(), 'T');
        assert_eq!(cursor.goto_line_begin(), 24);
        assert_eq!(cursor.next_codepoint().unwrap(), 'T');
        cursor.set(3);
        assert_eq!(cursor.goto_line_begin(), 0);
        assert_eq!(cursor.next_codepoint().unwrap(), 'F');
    }

    #[test]
    fn prev_line() {
        let rope = Rope::from_str("First line\nSecond line\r\nThird line\nFour").unwrap();
        let mut cursor = Cursor::new(&rope, rope.len());

        assert_eq!(cursor.goto_prev_line(), 24);
        assert_eq!(cursor.next_codepoint().unwrap(), 'T');

        assert_eq!(cursor.goto_prev_line(), 11);
        assert_eq!(cursor.next_codepoint().unwrap(), 'S');

        assert_eq!(cursor.goto_prev_line(), 0);
        assert_eq!(cursor.next_codepoint().unwrap(), 'F');
    }

    #[test]
    fn line_begin_pos() {
        let rope = Rope::from_str("One\nTwo\nThi\nFo4\nFiv\nSix\n7en").unwrap();
        let mut cursor = Cursor::new(&rope, 13);

        assert_eq!(cursor.line_beginning_position(None), 12);
        assert_eq!(cursor.line_beginning_position(Some(1)), 12);
        assert_eq!(cursor.line_beginning_position(Some(2)), 16);
        assert_eq!(cursor.line_beginning_position(Some(3)), 20);

        assert_eq!(cursor.line_beginning_position(Some(0)), 8);
        assert_eq!(cursor.line_beginning_position(Some(-1)), 4);
        assert_eq!(cursor.line_beginning_position(Some(-2)), 0);
    }

}
