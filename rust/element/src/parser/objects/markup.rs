use memchr::{memchr, memmem};

use super::super::Parser;
use crate::{
    data::{EntityData, NodeId, Syntax, SyntaxNode},
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    pub(super) fn try_parse_entity(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || bytes[0] != b'\\' {
            return None;
        }

        // Entity name: backslash followed by word chars
        let mut end = 1;
        while end < bytes.len() {
            let c = bytes[end];
            // Entity names are alphanumeric or certain special chars
            if c.is_ascii_alphanumeric() || c == b'-' {
                end += 1;
            } else {
                break;
            }
        }

        if end <= 1 {
            return None;
        }

        // Check post-char: end of text, whitespace, or punctuation
        let valid_post = end >= bytes.len()
            || bytes[end] == b' '
            || bytes[end] == b'\t'
            || bytes[end] == b'\n'
            || (end < bytes.len() && !bytes[end].is_ascii_alphanumeric());

        if !valid_post {
            return None;
        }

        let entity_name = &text[1..end];
        let entity_data = match EntityData::new(entity_name) {
            Some(d) => d,
            None => {
                // The greedy scan may have included digits after an
                // alphabetic entity name (e.g. `\le100Mb` → "le100Mb").
                // Try the alphabetic prefix only.
                let alpha_end = 1 + bytes[1..end]
                    .iter()
                    .position(|&b| !b.is_ascii_alphabetic())
                    .unwrap_or(end - 1);
                if alpha_end < end {
                    let d = EntityData::new(&text[1..alpha_end])?;
                    end = alpha_end;
                    d
                } else {
                    return None;
                }
            }
        };

        // Per Emacs' org-element-entity-parser: when the entity name is
        // followed by `{}`, the empty brace pair is consumed as part of
        // the entity (use-brackets-p=true). This terminator lets
        // `\vert{}def` render without inserting a space between `\vert`
        // and `def`.
        if bytes.get(end) == Some(&b'{') && bytes.get(end + 1) == Some(&b'}') {
            end += 2;
        }

        // Absorb trailing spaces/tabs as post-blank, matching Emacs.
        let post_blank = bytes[end..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Entity(self.bump.alloc(entity_data)),
                (start, start + end + post_blank),
                self.bump,
            )
            .post_blank(post_blank)
            .build(),
        );

        Some((node, end + post_blank))
    }

    pub(super) fn try_parse_line_break(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 2 || bytes[0] != b'\\' || bytes[1] != b'\\' {
            return None;
        }
        // Reject when preceded by a backslash, so `\\\` and `\\\\` runs do not
        // yield a line break.
        if start > 0 && self.input.as_bytes()[start - 1] == b'\\' {
            return None;
        }
        // Only spaces/tabs may separate the `\\` from the end of the line.
        let mut i = 2;
        while bytes.get(i).is_some_and(|&b| b == b' ' || b == b'\t') {
            i += 1;
        }
        match bytes.get(i) {
            None => {}              // end of buffer
            Some(&b'\n') => i += 1, // consume the newline
            _ => return None,       // non-whitespace before EOL → not a line break
        }
        let node = self
            .arena
            .alloc(SyntaxNode::new(Syntax::LineBreak, (start, start + i), self.bump).build());
        Some((node, i))
    }

    pub(super) fn try_parse_latex_fragment(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || bytes[0] != b'\\' {
            return None;
        }
        let len = bytes.len();

        // Scan command name: one or more letters, optionally ending with a
        // single '*' (e.g. `\section*`). Digits terminate the command, so
        // `\Office16` is the fragment `\Office` followed by plain-text `16`.
        let mut pos = 1;
        if pos >= len || !bytes[pos].is_ascii_alphabetic() {
            return None;
        }
        pos += 1;
        while pos < len && bytes[pos].is_ascii_alphabetic() {
            pos += 1;
        }
        if pos < len && bytes[pos] == b'*' {
            pos += 1;
        }

        // If the command ends at a digit, the full `\NAME123` may be a
        // known entity (e.g. `\frac12`, `\frac34`, `\there4`).  Extend
        // through digits and defer to the entity parser when the longer
        // name is recognised.  Otherwise the LaTeX fragment parser would
        // steal the alpha prefix (`\frac`) and the entity parser never
        // gets a turn.
        if pos < len && bytes[pos].is_ascii_digit() {
            let mut dig_end = pos;
            while dig_end < len && bytes[dig_end].is_ascii_digit() {
                dig_end += 1;
            }
            if EntityData::new(&text[1..dig_end]).is_some() {
                return None;
            }
        }

        // Per Emacs' entity regex (org-element-entity-parser), `\NAME`
        // followed by `{}` is parsed as an Entity with `use-brackets-p=t`,
        // not as a LaTeX fragment with an empty brace argument. Defer to
        // the entity parser in that case so `\vert{}def` becomes
        // Entity + PlainText instead of LatexFragment.
        if pos + 1 < len
            && bytes[pos] == b'{'
            && bytes[pos + 1] == b'}'
            && EntityData::new(&text[1..pos]).is_some()
        {
            return None;
        }

        let mut has_args = false;

        // Skip optional bracket argument(s) — Emacs' regex matches
        // multiple consecutive `[...]` groups (e.g. `\command[opt1][opt2]`).
        while pos < len && bytes[pos] == b'[' {
            has_args = true;
            pos += 1;
            let mut depth = 1;
            while pos < len && depth > 0 {
                if bytes[pos] == b'[' {
                    depth += 1;
                } else if bytes[pos] == b']' {
                    depth -= 1;
                }
                pos += 1;
            }
            if depth != 0 {
                return None;
            }
        }

        // Consume one or more brace groups {...}
        if pos < len && bytes[pos] == b'{' {
            has_args = true;
            loop {
                let mut depth = 0;
                while pos < len && bytes[pos] == b'{' {
                    depth += 1;
                    pos += 1;
                }
                if depth == 0 {
                    break;
                }
                while pos < len && depth > 0 {
                    if bytes[pos] == b'{' {
                        depth += 1;
                    } else if bytes[pos] == b'}' {
                        depth -= 1;
                    }
                    pos += 1;
                }
                if depth != 0 {
                    return None;
                }
            }
        }

        if !has_args {
            // Bare command — check if this is a known entity; if so, let the
            // entity parser handle it. Only match non-entity bare commands.
            if EntityData::new(&text[1..pos]).is_some() {
                return None;
            }
        }

        let consumed = pos; // position after the command name / closing '}'
        let fragment_text = &text[..consumed];

        let post_blank = text[consumed..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total_consumed = consumed + post_blank;

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::LatexFragment(fragment_text),
                (start, start + total_consumed),
                self.bump,
            )
            .build(),
        );

        Some((node, total_consumed))
    }

    pub(super) fn try_parse_latex_math(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return None;
        }

        let after_fragment = match bytes[0] {
            b'\\' => {
                if bytes.len() < 2 {
                    return None;
                }
                match bytes[1] {
                    b'(' => {
                        // \(...\)
                        let pos = memmem::find(&text.as_bytes()[2..], b"\\)")?;
                        2 + pos + 2
                    }
                    b'[' => {
                        // \[...\]
                        let pos = memmem::find(&text.as_bytes()[2..], b"\\]")?;
                        2 + pos + 2
                    }
                    _ => return None,
                }
            }
            b'$' => {
                if bytes.len() < 2 {
                    return None;
                }
                if bytes[1] == b'$' {
                    // $$...$$
                    let pos = memmem::find(&text.as_bytes()[2..], b"$$")?;
                    2 + pos + 2
                } else {
                    // $...$
                    if start > 0 && self.input.as_bytes()[start - 1] == b'$' {
                        return None;
                    }
                    match bytes[1] {
                        b' ' | b'\t' | b'\n' | b',' | b'.' | b';' => return None,
                        _ => {}
                    }
                    let pos = memchr(b'$', text.as_bytes().get(1..)?)?;
                    let closing = 1 + pos;
                    if closing <= 1 {
                        return None;
                    }
                    match bytes.get(closing - 1) {
                        Some(&b' ' | &b'\t' | &b'\n' | &b',' | &b'.') => return None,
                        _ => {}
                    }
                    let after = closing + 1;
                    if after < bytes.len() {
                        // Emacs syntax classes for valid post-chars: whitespace,
                        // punctuation (\\s.), brackets, and string quotes — but
                        // NOT symbol-characters like + - * / = # @ % ^ ~.
                        // Note: org-mode makes < and > bracket-pair syntax,
                        // so they ARE valid post-chars.
                        match bytes[after] {
                            b' ' | b'\t' | b'\n' | b'.' | b',' | b';' | b':' | b'!' | b'?'
                            | b'(' | b'[' | b'{' | b'<' | b')' | b']' | b'}' | b'>' | b'"'
                            | b'\'' => {}
                            _ => return None,
                        }
                    }
                    closing + 1
                }
            }
            _ => return None,
        };

        let fragment_text = &text[..after_fragment];

        let post_blank = text[after_fragment..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total_consumed = after_fragment + post_blank;

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::LatexFragment(fragment_text),
                (start, start + total_consumed),
                self.bump,
            )
            .build(),
        );

        Some((node, total_consumed))
    }
}
