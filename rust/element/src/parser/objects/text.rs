use memchr::{memchr, memchr2, memchr3};

use super::super::Parser;
use crate::{
    data::{NodeId, Syntax, SyntaxNode},
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    pub(super) fn try_parse_plain_text(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let mut consume = self.scan_plain_text_end(text.as_bytes());
        if consume == 0 {
            return None;
        }

        // Check if a radio link starts within the plain text range.
        // If so, truncate the plain text so the radio link is parsed
        // on the next iteration of the object loop.
        if !self.radio_targets.is_empty() {
            if let Some(link_start) = self.find_radio_link_start(text, start, consume) {
                consume = link_start;
                if consume == 0 {
                    return None;
                }
            }
        }

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::PlainText(&text[..consume]),
                (start, start + consume),
                self.bump,
            )
            .build(),
        );
        Some((node, consume))
    }

    /// Find the first occurrence of any radio target within `text[..limit]`.
    /// Returns the byte offset within `text` where a radio link starts, or
    /// `None` if no radio link is found.
    fn find_radio_link_start(&self, text: &str, start: usize, limit: usize) -> Option<usize> {
        let bytes = text.as_bytes();
        let mut pos = 0;

        while pos < limit {
            // Word boundary before: must be non-alphanumeric or start of buffer
            let abs_pos = start + pos;
            if abs_pos > 0 {
                let prev = self.input.as_bytes()[abs_pos - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' {
                    // Advance past this byte to the next char boundary
                    // (text may contain multi-byte UTF-8 characters)
                    pos += 1;
                    while pos < limit && !text.is_char_boundary(pos) {
                        pos += 1;
                    }
                    continue;
                }
            }

            // Check each target starting at this position
            for target in &self.radio_targets {
                if let Some(match_end) = Self::match_radio_target_text(&text[pos..], target) {
                    // Word boundary after: byte at `match_end` must be
                    // non-alphanumeric or end of buffer.
                    let next_pos = pos + match_end;
                    if next_pos < limit {
                        let next = bytes[next_pos];
                        if next.is_ascii_alphanumeric() || next == b'_' {
                            continue;
                        }
                    }

                    // Found a radio link starting at `pos`
                    return Some(pos);
                }
            }

            pos += 1;
            while pos < limit && !text.is_char_boundary(pos) {
                pos += 1;
            }
        }

        None
    }

    /// If `bytes` begins with one of the environment's plain-link types followed
    /// by `:`, return the length of that `type:` prefix.
    #[inline]
    pub(super) fn proto_len(&self, bytes: &[u8]) -> Option<usize> {
        self.environment.link_types().iter().find_map(|ty| {
            let t = ty.as_bytes();
            (bytes.starts_with(t) && bytes.get(t.len()) == Some(&b':')).then_some(t.len() + 1)
        })
    }

    /// Scan `bytes` for the first position that ends a plain-text run,
    /// returning the number of bytes that belong to the run.  Returns zero
    /// when no plain-text bytes are available at the start of `bytes`.
    #[inline]
    fn scan_plain_text_end(&self, bytes: &[u8]) -> usize {
        let link_start_bytes = self.environment.link_start_bytes();
        let mut is_link_start = [false; 256];
        for &c in link_start_bytes {
            is_link_start[c as usize] = true;
        }
        let find_link = |from: &[u8]| from.iter().position(|&b| is_link_start[b as usize]);

        let p1 = memchr3(b'[', b'<', b'\\', bytes);
        let p_dollar = memchr(b'$', bytes);
        let p_lbrace = memchr(b'{', bytes);
        let mut p2 = memchr2(b'*', b'/', bytes);
        let mut p3 = memchr3(b'+', b'=', b'~', bytes);
        let mut p5 = memchr2(b'_', b'^', bytes);
        let mut pc = memchr(b'c', bytes);
        let mut pl = find_link(bytes);

        loop {
            let i = match [p1, p_dollar, p_lbrace, p2, p3, p5, pc, pl]
                .iter()
                .copied()
                .flatten()
                .min()
            {
                None => return bytes.len(),
                Some(pos) => pos,
            };
            let b = bytes[i];

            if matches!(b, b'[' | b'<' | b'\\' | b'$' | b'{') {
                return i;
            }
            if matches!(b, b'*' | b'/' | b'+' | b'=' | b'~')
                && i > 0
                && super::super::is_pre_char(bytes[i - 1])
            {
                return i;
            }
            if matches!(b, b'c') && i > 0 && super::super::is_pre_char(bytes[i - 1]) {
                return i;
            }
            if matches!(b, b'_' | b'^') && i > 0 {
                // If the underscore is the `_` in `src_`, backtrack so the object
                // loop dispatches on `s` for inline-src-block instead of leaving
                // `_` for the subscript parser.
                if i >= 4 && bytes[i - 3] == b's' && bytes[i - 2] == b'r' && bytes[i - 1] == b'c' {
                    return i - 3;
                }
                return i;
            }
            // A plain link must sit at a word boundary. Accept any non-ASCII byte
            // as a pre-char (CJK etc.). Unlike emphasis, `'` is NOT a valid
            // pre-char for plain links.
            if (i == 0
                || (super::super::is_pre_char(bytes[i - 1]) && bytes[i - 1] != b'\'')
                || bytes[i - 1] >= 0x80)
                && self.proto_len(&bytes[i..]).is_some()
            {
                return i;
            }

            let next = i + 1;
            let rest = &bytes[next..];
            match b {
                b'*' | b'/' => p2 = memchr2(b'*', b'/', rest).map(|r| next + r),
                b'+' | b'=' | b'~' => p3 = memchr3(b'+', b'=', b'~', rest).map(|r| next + r),
                b'_' | b'^' => p5 = memchr2(b'_', b'^', rest).map(|r| next + r),
                b'c' => pc = memchr(b'c', rest).map(|r| next + r),
                _ => pl = find_link(rest).map(|r| next + r),
            }
        }
    }

    /// Match `text` against a radio target string with case-insensitive
    /// comparison and whitespace-run collapsing (Emacs `\\s-+` semantics).
    /// Returns bytes consumed from `text` on success, or `None`.
    #[inline]
    pub(super) fn match_radio_target_text(text: &str, target: &str) -> Option<usize> {
        let t = target.as_bytes();
        let s = text.as_bytes();
        let (mut ti, mut si) = (0, 0);

        while ti < t.len() && si < s.len() {
            let tc = t[ti];
            let sc = s[si];

            if tc == b' ' {
                if !sc.is_ascii_whitespace() {
                    return None;
                }
                ti += 1;
                si += 1;
                while ti < t.len() && t[ti] == b' ' {
                    ti += 1;
                }
                while si < s.len() && s[si].is_ascii_whitespace() {
                    si += 1;
                }
            } else if tc.eq_ignore_ascii_case(&sc) {
                ti += 1;
                si += 1;
            } else if sc.is_ascii_whitespace() {
                si += 1;
            } else {
                return None;
            }
        }

        if ti == t.len() {
            Some(si)
        } else {
            None
        }
    }
}
