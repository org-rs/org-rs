use memchr::memchr2;

use super::super::Parser;
use crate::{
    data::{Brackets, BumpVec, NodeId, ScriptFlags, ScriptKind, Syntax, SyntaxNode, SyntaxT},
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    pub(super) fn try_parse_bold(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'*', SyntaxT::Bold)
    }

    pub(super) fn try_parse_italic(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'/', SyntaxT::Italic)
    }

    pub(super) fn try_parse_underline(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'_', SyntaxT::Underline)
    }

    pub(super) fn try_parse_strikethrough(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'+', SyntaxT::StrikeThrough)
    }

    pub(super) fn try_parse_code(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'~', SyntaxT::Code)
    }

    pub(super) fn try_parse_verbatim(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'=', SyntaxT::Verbatim)
    }

    pub(super) fn try_parse_script(
        &mut self,
        text: &'a str,
        start: usize,
        kind: ScriptKind,
    ) -> Option<(NodeId, usize)> {
        // Subscript/superscript requires a non-whitespace character
        // before the marker. This matches Emacs' behavior.
        let _ = start
            .checked_sub(1)
            .and_then(|i| self.input.as_bytes().get(i))
            .filter(|&&b| !matches!(b, b' ' | b'\t' | b'\n'))?;

        let bytes = text.as_bytes();

        let (brackets, consumed) = match bytes.get(1)? {
            b'{' => {
                let rest = bytes.get(2..)?;
                let close = memchr::memchr(b'}', rest)
                    .filter(|&c| !rest[..c].iter().any(|&b| b == b'{' || b == b'\n'))?;
                (Brackets::Bracketed, 2 + close + 1)
            }
            b'*' => (Brackets::Bare, 2),
            _ => {
                let content = &text[1..];
                // Bare subscript: matches Emacs regex
                //   [+-]?[[:alnum:].,\\]*[[:alnum:]]
                // The content may include `.`, `,`, `\` but MUST end with
                // an alphanumeric character (backtracking if necessary).
                // Rust's is_alphanumeric() matches CJK ideographs just
                // like Emacs' [[:alnum:]], so no special treatment needed.
                let mut last_alnum = None;
                for (i, ch) in content.char_indices() {
                    if ch == '+' || ch == '-' {
                        if i != 0 {
                            break;
                        }
                        continue;
                    }
                    if ch.is_alphanumeric() {
                        last_alnum = Some(i + ch.len_utf8());
                    } else if matches!(ch, '.' | ',') {
                        // continue scanning — may or may not be followed by alnum
                    } else if self.environment.emacs_compliance() && ch == '\\' {
                        // Emacs regex [+-]?[[:alnum:].,\\]*[[:alnum:]]
                        // allows backslash inside bare subscripts.
                        // Outside compliance mode we stop here —
                        // backslash content inside a subscript is
                        // almost never intended.
                    } else if (0x0300..=0x0D7F).contains(&(ch as u32)) {
                        // Accept combining marks (Unicode categories Mn, Mc)
                        // that are not captured by is_alphanumeric().
                        // Emacs' [[:alnum:]] includes these as word constituents
                        // in its syntax table, allowing e.g. the Bengali virama
                        // U+09CD ্ to appear inside a bare subscript.  The range
                        // U+0300–U+0D7F covers Combining Diacritical Marks and
                        // all Brahmic Indic-script blocks.
                        // Continue scanning without updating last_alnum so the
                        // subscript MUST still end on a true alphanumeric char
                        // (matching Emacs' regex).
                    } else {
                        break;
                    }
                }
                match last_alnum {
                    Some(len) => (Brackets::Bare, 1 + len),
                    None => return None,
                }
            }
        };

        let (content_begin, content_end) = match brackets {
            Brackets::Bare => (start + 1, start + consumed),
            Brackets::Bracketed => (start + 2, start + consumed - 1),
        };

        // Emacs parses objects inside subscript/superscript content
        // (e.g. `\n` inside `_SRC\n"` is a LaTeX macro).  Match this
        // in compliance mode.  Outside compliance mode the content is
        // treated as plain text for simplicity.
        let compliance = self.environment.emacs_compliance();
        let content_nonempty = content_begin < content_end;
        let script_children: BumpVec<'b, NodeId> = if compliance && content_nonempty {
            self.parse_objects((content_begin, content_end), |_| true)
        } else if content_nonempty {
            let plain_text = self.arena.alloc(
                SyntaxNode::new(
                    Syntax::PlainText(&self.input[content_begin..content_end]),
                    (content_begin, content_end),
                    self.bump,
                )
                .build(),
            );
            let mut v: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
            v.push(plain_text);
            v
        } else {
            BumpVec::new_in(self.bump)
        };

        // Absorb trailing whitespace as post-blank, matching Emacs
        // org-element behaviour for bare sub/superscripts.
        let post_blank = bytes[consumed..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let flags = ScriptFlags::new(kind, brackets);
        let node = self.arena.alloc_with_children(
            SyntaxNode::new(
                Syntax::Script(flags),
                (start, start + consumed + post_blank),
                self.bump,
            )
            .post_blank(post_blank)
            .build(),
            script_children,
        );
        Some((node, consumed + post_blank))
    }

    fn parse_emphasis_marker(
        &mut self,
        text: &str,
        start: usize,
        marker: u8,
        syntax: SyntaxT,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || bytes[0] != marker {
            return None;
        }

        // Check PRE condition: the marker must be at the start of the content
        // region being parsed (start of buffer, link description, footnote,
        // nested emphasis, …) or immediately preceded by a valid pre-character
        // (whitespace or one of `-([{'"`). Without this, the `_` after `BEGIN`
        // in `(#+BEGIN_... and #+END_...)` is wrongly read as an underline.
        let valid_pre = start == self.object_region_start
            || super::super::is_pre_char(self.input.as_bytes()[start - 1]);
        if !valid_pre {
            return None;
        }

        // Need at least: marker + 1 content + closing marker
        if text.len() < 3 {
            return None;
        }

        // First content char must be non-whitespace.
        // Emacs treats NBSP (U+00A0), ZWS (U+200B) and other Unicode
        // whitespace as "space" for emphasis boundary checks (via syntax
        // table classification).
        if bytes[1] == b' ' || bytes[1] == b'\t' || bytes[1] == b'\n' {
            return None;
        }
        if let Some(c) = text[1..].chars().next() {
            if c == super::super::NBSP || c == super::super::ZWS || c.is_ascii_whitespace() {
                return None;
            }
        }

        let mut found_close = None;
        let mut pos = 2;

        // Search for closing marker.  Unlike the Emacs regex-based parser
        // this scans byte-by-byte, but like Emacs there is no limit on
        // the number of newlines inside emphasis markers.
        while pos < text.len() {
            match memchr2(b'\n', marker, &bytes[pos..]) {
                Some(offset) => {
                    let i = pos + offset;
                    if bytes[i] == b'\n' {
                        pos = i + 1;
                    } else {
                        // Found marker - last content char must be non-whitespace.
                        // Emacs also treats NBSP (U+00A0), ZWS (U+200B) etc. as
                        // whitespace here via syntax-table classification.
                        let prev = bytes[i - 1];
                        if prev != b' ' && prev != b'\t' && prev != b'\n' {
                            let content = &text[1..i];
                            let last_char = content.chars().last();
                            let is_ws = last_char.map_or(false, |c| {
                                c == super::super::NBSP
                                    || c == super::super::ZWS
                                    || c.is_ascii_whitespace()
                            });
                            if !is_ws {
                                let valid_post =
                                    i + 1 >= text.len() || super::super::is_post_char(bytes[i + 1]);
                                if valid_post {
                                    found_close = Some(i);
                                    break;
                                }
                            }
                        }
                        pos = i + 1;
                    }
                }
                None => break,
            }
        }

        let close = found_close?;

        let content_location = crate::data::Interval {
            start: start + 1,
            end: start + close,
        };
        let content = &self.input[content_location.start..content_location.end];

        let (data, children) = match syntax {
            SyntaxT::Bold => (Syntax::Bold, self.parse_objects(content_location, |_| true)),
            SyntaxT::Italic => (
                Syntax::Italic,
                self.parse_objects(content_location, |_| true),
            ),
            SyntaxT::Underline => (
                Syntax::Underline,
                self.parse_objects(content_location, |_| true),
            ),
            SyntaxT::StrikeThrough => (
                Syntax::StrikeThrough,
                self.parse_objects(content_location, |_| true),
            ),
            SyntaxT::Code => (Syntax::Code(content), BumpVec::new_in(self.bump)),
            SyntaxT::Verbatim => (Syntax::Verbatim(content), BumpVec::new_in(self.bump)),
            _ => return None,
        };

        let post_blank = bytes
            .get(close + 1..)
            .map(|rest| {
                rest.iter()
                    .take_while(|&&b| b == b' ' || b == b'\t')
                    .count()
            })
            .unwrap_or(0);

        let node = self.arena.alloc_with_children(
            SyntaxNode::new(data, (start, start + close + 1 + post_blank), self.bump)
                .content(content_location)
                .build(),
            children,
        );

        Some((node, close + 1 + post_blank))
    }
}
