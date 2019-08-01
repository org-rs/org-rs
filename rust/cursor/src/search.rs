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


#[cfg(test)]
mod test {
    use crate::{Cursor, Char};
    use crate::search::SearchCursor;
    use crate::cursor::LexemeCursor;
    use regex::Regex;

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
