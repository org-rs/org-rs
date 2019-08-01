use crate::{Cursor, Interval, Char};
use regex::Regex;
use crate::cursor::{MetricCursor, LexemeCursor};

pub trait SearchCursor {

    fn skip_chars_forward(&mut self, str: &str, limit: Option<usize>) -> usize;
    fn skip_chars_backward(&mut self, str: &str, limit: Option<usize>) -> usize;

    fn search_forward(&mut self, str: &str, bound: Option<usize>, count: Option<usize>) -> Option<usize>;
    fn re_search_forward(&mut self, re: &Regex, bound: Option<usize>) -> Option<Interval>;
}

impl<'a> SearchCursor for Cursor<'a> {
    /// Moves cursor forward, stopping before a char not in str, or at position limit.
    fn skip_chars_forward(&mut self, str: &str, limit: Option<usize>) -> usize {
        let pos = self.pos();
        let limit = match limit {
            Some(lim) => lim,
            _ => self.data_len(),
        };

        if pos >= limit {
            return 0;
        }

        let mut count = 0;
        while let Some(c) = self.get_lnext::<Char>() {
            if !str.contains(c) {
                self.mprev::<Char>();
                return count;
            }
            if count + pos > limit {
                self.mprev::<Char>();
                return count;
            }
            count += 1;
        }
        count
    }

    /// Move point backward, stopping after a char not in str, or at `limit`
    /// `limit` - is an absolute buffer position
    /// Returns the distance traveled.
    ///
    /// Difference with Emacs variant is that emacs returns negative number
    ///
    /// (skip-chars-backward STRING &optional LIM)
    fn skip_chars_backward(&mut self, str: &str, limit: Option<usize>) -> usize {
        let limit = match limit {
            Some(lim) => lim,
            _ => 0,
        };

        if self.pos() <= limit {
            return 0;
        }

        let mut count = 0;
        while let Some(c) = self.get_lprev::<Char>() {
            if !str.contains(c) {
                self.mnext::<Char>();
                return count;
            }
            if self.pos() < limit {
                self.mnext::<Char>();
                return count;
            }
            count += 1;
        }
        count
    }


    /// Search forward from point to str. Sets point to the end of the
    /// occurence found and returns point. bound is a position in the
    /// buffer. The match found must not end after that position. If
    /// None then search to end of the buffer. If count is specified,
    /// find the countth occurence. If countth occurence is not found
    /// None is returned. If count is not provided then 1 is used as
    /// count. Note that searching backward is not supported like it
    /// is in the elisp equivalent.
    fn search_forward(
        &mut self,
        str: &str,
        bound: Option<usize>,
        count: Option<usize>,
    ) -> Option<usize> {
        let count = match count {
            Some(count) => count,
            _ => 1,
        };

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

    ///
    /// Search forward from point for regular expression REGEXP.
    /// Set point to the end of the occurrence found, and return match Interval
    /// with absolute positions.
    /// Original implementation returned cursor position and modified global variables
    /// with match data
    ///
    /// The optional second argument BOUND is a buffer position that bounds
    ///   the search.  The match found must not end after that position.  A
    ///   value of nil means search to the end of the accessible portion of
    ///   the buffer.
    /// elisp:`(re-search-forward REGEXP &optional BOUND NOERROR COUNT)`
    fn re_search_forward(&mut self, re: &Regex, bound: Option<usize>) -> Option<Interval> {
        let end = bound.unwrap_or(self.data.len());

        if end <= self.pos {
            return None;
        }

        // Set point to the end of the occurrence found, and return point.
        match re.find(&self.data[self.pos..end]) {
            None => None,
            Some(m) => {
                let res = Interval::new(self.pos + m.start(), self.pos + m.end());
                self.set(self.pos + m.end());
                Some(res)
            }
        }
    }
}
