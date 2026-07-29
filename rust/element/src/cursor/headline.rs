use memchr::{memmem, memrchr};

use super::{Cursor, LinesMetric};

impl<'a> Cursor<'a> {
    /// Possibly moves cursor to the beginning of the next headline.
    /// Corresponds to `outline-next-heading` in Emacs.
    /// If a next headline is found, returns its start position.
    #[inline]
    pub fn next_headline(&mut self) -> Option<usize> {
        self.next::<LinesMetric>();
        let beg = self.pos();
        let bytes = self.data.as_bytes();

        if let Some(&b'*') = bytes.get(beg) {
            let n = bytes[beg..].iter().take_while(|&&b| b == b'*').count();
            if n > 0 && bytes.get(beg + n).is_some_and(|&b| b == b' ' || b == b'\t') {
                return Some(beg);
            }
        }

        for offset in memmem::find_iter(&bytes[beg..], b"\n*") {
            let line_start = beg + offset + 1;
            let n = bytes[line_start..]
                .iter()
                .take_while(|&&b| b == b'*')
                .count();
            if n > 0
                && bytes
                    .get(line_start + n)
                    .is_some_and(|&b| b == b' ' || b == b'\t')
            {
                self.pos = line_start;
                return Some(line_start);
            }
        }
        None
    }

    /// Return true if cursor is on a headline.
    /// Corresponds to `org-at-heading-p`.
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
        n > 0 && tail.get(n).is_some_and(|&b| b == b' ' || b == b'\t')
    }

    /// Return true if the cursor is on an inline-task line: a headline-like
    /// line with at least 15 stars (`org-inlinetask-min-level`).
    #[inline]
    pub fn on_inline_task(&self) -> bool {
        let bytes = self.data.as_bytes();
        let line_start = if self.is_bol() {
            self.pos
        } else {
            memrchr(b'\n', &bytes[..self.pos]).map_or(0, |i| i + 1)
        };
        let tail = &bytes[line_start..];
        let n = tail.iter().take_while(|&&b| b == b'*').count();
        n >= 15 && tail.get(n).is_some_and(|&b| b == b' ' || b == b'\t')
    }

    /// Position of the next *real* headline — one below the inline-task
    /// threshold of 15 stars. Inline-task lines are skipped.
    #[inline]
    pub fn next_real_headline(&mut self, limit: usize) -> Option<usize> {
        self.next::<LinesMetric>();
        let beg = self.pos();
        let end = limit.min(self.data.len());
        if beg >= end {
            return None;
        }
        let bytes = self.data.as_bytes();

        if let Some(&b'*') = bytes.get(beg) {
            let n = bytes[beg..end].iter().take_while(|&&b| b == b'*').count();
            if n > 0 && n < 15 && bytes.get(beg + n).is_some_and(|&b| b == b' ' || b == b'\t') {
                return Some(beg);
            }
        }

        for offset in memmem::find_iter(&bytes[beg..end], b"\n*") {
            let line_start = beg + offset + 1;
            if line_start >= end {
                break;
            }
            let n = bytes[line_start..end]
                .iter()
                .take_while(|&&b| b == b'*')
                .count();
            if n > 0
                && n < 15
                && bytes
                    .get(line_start + n)
                    .is_some_and(|&b| b == b' ' || b == b'\t')
            {
                self.pos = line_start;
                return Some(line_start);
            }
        }
        None
    }

    /// If the byte at `pos` in this cursor's data is the start of an
    /// inline-task END marker (`\*{15,}[ \t]+END[ \t]*\n?`), return the
    /// byte offset just past that line. Otherwise return `None`.
    /// Does not move the cursor.
    #[inline]
    pub fn inlinetask_end_at(&self, pos: usize) -> Option<usize> {
        let bytes = self.data.as_bytes();
        let stars = bytes[pos..].iter().take_while(|&&b| b == b'*').count();
        if stars < 15 {
            return None;
        }
        let mut i = pos + stars;
        let ws = bytes[i..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
        if ws == 0 {
            return None;
        }
        i += ws;
        if !bytes
            .get(i..i + 3)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"END"))
        {
            return None;
        }
        i += 3;
        i += bytes[i..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
        match bytes.get(i) {
            None => Some(i),
            Some(&b'\n') => Some(i + 1),
            _ => None,
        }
    }

    /// Scan forward from `from` in this cursor's data to find the byte
    /// position of the next headline at the same or higher level, or `limit`
    /// if none is found. Does not move the cursor.
    #[inline]
    pub fn find_headline_end(&self, from: usize, level: usize, limit: usize) -> usize {
        let bytes = self.data.as_bytes();

        if from >= limit {
            return limit;
        }

        if let Some(&b'*') = bytes.get(from) {
            let stars = bytes[from..limit]
                .iter()
                .take_while(|&&b| b == b'*')
                .count();
            if stars <= level
                && bytes
                    .get(from + stars)
                    .is_some_and(|&b| b == b' ' || b == b'\t')
            {
                return from.min(limit);
            }
        }

        for offset in memmem::find_iter(&bytes[from..limit], b"\n*") {
            let line_start = from + offset + 1;
            let stars = bytes[line_start..]
                .iter()
                .take_while(|&&b| b == b'*')
                .count();
            if stars <= level
                && bytes
                    .get(line_start + stars)
                    .is_some_and(|&b| b == b' ' || b == b'\t')
            {
                return line_start.min(limit);
            }
        }

        limit
    }

    /// Collect all top-level headline start positions (1–8 stars) in `input`.
    /// This is an associated function (no cursor state needed).
    #[inline]
    pub fn find_all_headline_starts(input: &str) -> Vec<usize> {
        let bytes = input.as_bytes();
        let mut positions = Vec::new();

        if bytes.first() == Some(&b'*') {
            let stars = bytes.iter().take_while(|&&b| b == b'*').count();
            if (1..=8).contains(&stars)
                && bytes.get(stars).is_some_and(|&b| b == b' ' || b == b'\t')
            {
                positions.push(0);
            }
        }

        for offset in memmem::find_iter(bytes, b"\n*") {
            let pos = offset + 1;
            let tail = &bytes[pos..];
            let stars = tail.iter().take_while(|&&b| b == b'*').count();
            if (1..=8).contains(&stars) && tail.get(stars).is_some_and(|&b| b == b' ' || b == b'\t')
            {
                positions.push(pos);
            }
        }

        positions
    }
}
