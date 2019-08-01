use crate::cursor::{LexemeCursor, MetricCursor};
use crate::metrics::Metric;
use crate::{is_multiline_regex, Char, Cursor, Line};
use regex::{Captures, Match, Regex};

pub trait EmacsCursor<'a> {
    fn char_after(&mut self, offset: usize) -> Option<char>;
    fn skip_whitespace(&mut self) -> usize;
    fn looking_at(&self, re: &Regex) -> Option<Match<'a>>;
    fn capturing_at(&self, re: &Regex) -> Option<Captures<'a>>;
    fn is_bol(&self) -> bool;

    fn goto_line_begin(&mut self) -> usize;
    fn line_beginning_position(&mut self, n: Option<i32>) -> usize;
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

    /// Skip over space, tabs and newline characters
    /// Cursor position is set before next non-whitespace char
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

    /// Acts exactly as `looking_at` but returns Captures
    /// This is slower than simple regex search so if you don't need
    /// capture groups use `looking_at` for better performance
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

    /// Moves cursor to the beginning of the current line.
    /// Acts like "Home" button
    /// If cursor is already at the beginning of the line - nothing happens
    /// Returns the position of the cursor
    fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 && self.at_or_mprev::<Line>().is_none() {
            self.set(0);
        }
        self.pos()
    }

    /// Return the character position of the first character on the current line.
    /// If N is none then acts as `goto_line_begin`
    /// Otherwise moves forward N - 1 lines first.
    /// with N < 1 cursor will move to previous lines
    ///
    /// Corresponds to `line-beginning-position` in elisp
    /// This function does not move the cursor (does save-excursion)
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

    /// Return the character position of the last character on the current line.
    /// With argument N not nil or 1, move forward N - 1 lines first.
    /// If scan reaches end of buffer, return that position.
    ///
    /// Corresponds to `line-end-position` in elisp
    /// This function does not move the cursor (does save-excursion)
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
