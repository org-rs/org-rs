use crate::cursor::{LexemeCursor, MetricCursor};
use crate::metrics::Metric;
use crate::{is_multiline_regex, Char, Cursor, Line};
use regex::{Captures, Match, Regex};

pub trait EmacsCursor<'a> {
    fn char_after(&mut self, offset: usize) -> Option<char>;

    /// Skip over space, tabs and newline characters
    /// Cursor position is set before next non-whitespace char
    fn skip_whitespace(&mut self) -> usize;

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
    fn looking_at(&self, re: &Regex) -> Option<Match<'a>>;


    /// Acts exactly as `looking_at` but returns Captures
    /// This is slower than simple regex search so if you don't need
    /// capture groups use `looking_at` for better performance
    fn capturing_at(&self, re: &Regex) -> Option<Captures<'a>>;

    /// True if cursor is at the beginning of the buffer or
    /// on a newline
    fn is_bol(&self) -> bool;

    /// Moves cursor to the beginning of the current line.
    /// Acts like "Home" button
    /// If cursor is already at the beginning of the line - nothing happens
    /// Returns the position of the cursor
    fn goto_line_begin(&mut self) -> usize;


    /// Moves 1 line forward
    /// If there isn’t room, go as far as possible (no error).
    fn forward_line(&mut self);


    /// Moves 1 line backward
    /// If there isn’t room, go as far as possible (no error).
    fn backward_line(&mut self);


    /// Return the character position of the first character on the current line.
    /// If N is none then acts as `goto_line_begin`
    /// Otherwise moves forward N - 1 lines first.
    /// with N < 1 cursor will move to previous lines
    ///
    /// Corresponds to `line-beginning-position` in elisp
    /// This function does not move the cursor (does save-excursion)
    fn line_beginning_position(&mut self, n: Option<i32>) -> usize;


    /// Return the character position of the last character on the current line.
    /// With argument N not nil or 1, move forward N - 1 lines first.
    /// If scan reaches end of buffer, return that position.
    ///
    /// Corresponds to `line-end-position` in elisp
    /// This function does not move the cursor (does save-excursion)
    fn line_end_position(&mut self, n: Option<i32>) -> usize;
}

impl<'a> EmacsCursor<'a> for Cursor<'a> {
    // TODO refactor to use Metric
    fn char_after(&mut self, offset: usize) -> Option<char> {
        let pos = self.pos();
        self.set(offset);
        let result = self.get_lnext::<Char>();
        self.set(pos);
        return result;
    }

    fn skip_whitespace(&mut self) -> usize {
        while let Some(c) = self.get_lnext::<Char>() {
            if !(c.is_whitespace()) {
                self.lprev::<Char>();
                break;
            }
            // else {
            //     self.next::<Char>();
            // }
        }
        self.pos()
    }

    fn looking_at(&self, re: &Regex) -> Option<Match<'a>> {
        let end = if !is_multiline_regex(re.as_str()) {
            Line::next(self.data, self.pos)
                .map(|p| p - 1) // exclude '\n' from the string'
                .unwrap_or_else(|| self.data.len())
        } else {
            self.data.len()
        };
        re.find(&self.data[self.pos..end])
    }

    fn capturing_at(&self, re: &Regex) -> Option<Captures<'a>> {
        let end = if !is_multiline_regex(re.as_str()) {
            Line::next(self.data, self.pos)
                .map(|p| p - 1) // exclude '\n' from the string'
                .unwrap_or_else(|| self.data.len())
        } else {
            self.data.len()
        };

        re.captures(&self.data[self.pos..end])
    }

    fn is_bol(&self) -> bool {
        if self.pos() == 0 {
            true
        } else {
            self.is_boundary::<Line>()
        }
    }

    fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 && self.at_or_mprev::<Line>().is_none() {
            self.set(0);
        }
        self.pos()
    }

    fn forward_line(&mut self) {
        unimplemented!()
    }


    fn backward_line(&mut self) {
        unimplemented!()
    }

    fn line_beginning_position(&mut self, n: Option<i32>) -> usize {
        let pos = self.pos();
        match n {
            None | Some(1) => {
                Some(self.goto_line_begin());
            }

            Some(x) => {
                if x > 1 {
                    for _p in 0..x - 1 {
                        if self.mnext::<Line>().is_none() {
                            self.eof();
                            break;
                        }
                    }
                } else {
                    self.goto_line_begin();
                    if self.pos() != 0 {
                        for _ in 0..(x - 1).abs() {
                            if self.mprev::<Line>().is_none() {
                                self.bof();
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

    fn line_end_position(&mut self, n: Option<i32>) -> usize {
        let pos = self.pos();
        match n {
            None | Some(1) => {
                self.mnext::<Line>();
            }

            Some(x) => {
                if x > 1 {
                    for _ in 0..x {
                        if self.mnext::<Line>().is_none() {
                            self.eof()
                        }
                    }
                } else if self.pos() != 0 {
                    for _ in 0..=x.abs() {
                        if self.mprev::<Line>().is_none() {
                            break;
                        }
                    }
                }
            }
        }

        let result = self.mprev::<Char>().unwrap_or(0);
        self.set(pos);
        return result;
    }
}

#[cfg(test)]
mod test {
    use crate::{Cursor, Line, Char};
    use crate::emacs::EmacsCursor;
    use crate::cursor::{MetricCursor, LexemeCursor};

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
}
