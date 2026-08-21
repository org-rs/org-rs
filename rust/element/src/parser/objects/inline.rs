use super::super::Parser;
use crate::{
    data::{
        BumpVec, CitationData, FootnoteReferenceData, InlineBabelCallData, InlineSrcBlockData,
        Interval, MacroData, NodeId, StatisticsCookieData, Syntax, SyntaxNode, SyntaxT,
    },
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    pub(super) fn try_parse_citation(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if !bytes.starts_with(b"[cite") {
            return None;
        }
        // After "[cite", look for '/' (style) or ':' (end of keyword)
        let mut pos = 5; // len("[cite")
        let style = if bytes.get(pos) == Some(&b'/') {
            pos += 1;
            let style_start = pos;
            while let Some(&b) = bytes.get(pos) {
                if b == b':' {
                    break;
                }
                pos += 1;
            }
            if pos <= style_start {
                return None;
            }
            Some(&text[style_start..pos])
        } else {
            None
        };
        // Now at ':'
        if bytes.get(pos) != Some(&b':') {
            return None;
        }
        pos += 1; // skip ':'
                  // Scan for closing ']' — must contain at least one '@'
        let close_start = pos;
        while let Some(&b) = bytes.get(pos) {
            if b == b']' {
                break;
            }
            pos += 1;
        }
        if bytes.get(pos) != Some(&b']') {
            return None;
        }
        // Must contain at least one '@' before the closing ']'
        if !text[close_start..pos].contains('@') {
            return None;
        }
        let close = pos;
        let consumed = close + 1;
        let post_blank = text[consumed..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total_consumed = consumed + post_blank;
        let raw = &text[..consumed];
        let data = CitationData::new(raw, style);

        // Split the citation body (between ':' and ']') into ';'-separated
        // segments — ';' is the only reference delimiter in org-cite, and a
        // key (`@...`) can never contain one. Each segment that holds an '@'
        // key is a citation reference; key-less leading/trailing segments are
        // the citation's global prefix/suffix and produce no node.
        let mut seg_starts: BumpVec<'b, usize> = BumpVec::new_in(self.bump);
        seg_starts.push(close_start);
        for j in close_start..close {
            if bytes[j] == b';' {
                seg_starts.push(j + 1);
            }
        }
        let n = seg_starts.len();

        let mut children: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
        for i in 0..n {
            let seg_start = seg_starts[i];
            // The reference spans up to the next segment's start, so the
            // trailing ';' delimiter is included (matching Emacs).
            let seg_end = if i + 1 < n { seg_starts[i + 1] } else { close };
            let segment = &text[seg_start..seg_end];
            if !segment.contains('@') {
                continue;
            }
            let child = self.arena.alloc(
                SyntaxNode::new(
                    Syntax::CitationReference(segment),
                    (start + seg_start, start + seg_end),
                    self.bump,
                )
                .build(),
            );
            children.push(child);
        }

        let node = self.arena.alloc_with_children(
            SyntaxNode::new(
                Syntax::Citation(self.bump.alloc(data)),
                (start, start + total_consumed),
                self.bump,
            )
            .build(),
            children,
        );
        Some((node, total_consumed))
    }

    pub(super) fn try_parse_macro(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 6 || &bytes[0..3] != b"{{{" {
            return None;
        }

        let mut i = 3;
        // Macro name: must start with a letter
        if i >= bytes.len() || !bytes[i].is_ascii_alphabetic() {
            return None;
        }
        i += 1;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
        {
            i += 1;
        }

        let key = &text[3..i];

        // Optional parenthesised argument list.
        let mut args: Vec<&'a str> = Vec::new();
        let close: usize;

        if i < bytes.len() && bytes[i] == b'(' {
            i += 1;
            let arg_start = i;
            let mut depth = 1u32;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    i += 1;
                }
            }
            if depth != 0 {
                return None;
            }
            let arg_text = &text[arg_start..i];
            i += 1; // past ')'

            if !arg_text.is_empty() {
                for arg in arg_text.split(',') {
                    args.push(arg.trim());
                }
            }
        }

        // Expect closing }}}
        if i + 2 >= bytes.len() || &bytes[i..i + 3] != b"}}}" {
            return None;
        }
        close = i + 3;

        let value = &text[3..close - 3];

        // Absorb trailing spaces/tabs as post-blank.
        let post_blank = bytes[close..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Macro(self.bump.alloc(MacroData { key, args, value })),
                (start, start + close + post_blank),
                self.bump,
            )
            .post_blank(post_blank)
            .build(),
        );

        Some((node, close + post_blank))
    }

    pub(super) fn try_parse_inline_babel_call(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();

        // Must start with "call_"
        if !text.starts_with("call_") {
            return None;
        }

        let mut i = 5; // past "call_"

        // Function name: must start with a letter
        if i >= bytes.len() || !bytes[i].is_ascii_alphabetic() {
            return None;
        }
        i += 1;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_')
        {
            i += 1;
        }

        let call_name = &text[5..i];

        // Must have '('
        if i >= bytes.len() || bytes[i] != b'(' {
            return None;
        }
        i += 1;

        // Parse arguments until matching ')'
        let arg_start = i;
        let mut depth = 1u32;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                i += 1;
            }
        }
        if depth != 0 {
            return None;
        }
        let arguments = Some(&text[arg_start..i]);
        i += 1; // past ')'

        let close = i;

        // Absorb trailing spaces/tabs as post-blank.
        let post_blank = bytes[close..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let value = &text[..close];

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::InlineBabelCall(self.bump.alloc(InlineBabelCallData {
                    call: call_name,
                    arguments,
                    value,
                })),
                (start, start + close + post_blank),
                self.bump,
            )
            .post_blank(post_blank)
            .build(),
        );

        Some((node, close + post_blank))
    }

    pub(super) fn try_parse_inline_src_block(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();

        // Must start with "src_"
        if !text.starts_with("src_") {
            return None;
        }

        let mut i = 4; // past "src_"

        // Language name: at least one ascii letter
        if i >= bytes.len() || !bytes[i].is_ascii_alphabetic() {
            return None;
        }
        i += 1;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_')
        {
            i += 1;
        }

        let language = &text[4..i];

        // Optional header arguments: [:headers]
        let parameters = if bytes.get(i) == Some(&b'[') {
            let open = i;
            let mut depth = 1u32;
            i += 1;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    i += 1;
                }
            }
            if depth != 0 {
                return None;
            }
            i += 1;
            Some(&text[open + 1..i - 1])
        } else {
            None
        };

        // Body: `{...}` with balanced braces
        if i >= bytes.len() || bytes[i] != b'{' {
            return None;
        }
        let body_start = i + 1;
        let mut depth = 1u32;
        i += 1;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                i += 1;
            }
        }
        if depth != 0 {
            return None;
        }
        let value = &text[body_start..i];
        i += 1; // past '}'

        let consumed = i;

        // Absorb trailing whitespace as post-blank.
        let post_blank = bytes[consumed..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::InlineSrcBlock(self.bump.alloc(InlineSrcBlockData {
                    language,
                    parameters,
                    value,
                })),
                (start, start + consumed + post_blank),
                self.bump,
            )
            .post_blank(post_blank)
            .build(),
        );

        Some((node, consumed + post_blank))
    }

    pub(super) fn try_parse_statistics_cookie(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.first() != Some(&b'[') {
            return None;
        }
        let mut pos = 1;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        match bytes.get(pos) {
            Some(&b'%') => pos += 1,
            Some(&b'/') => {
                pos += 1;
                while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
                    pos += 1;
                }
            }
            _ => return None,
        }
        if bytes.get(pos) != Some(&b']') {
            return None;
        }
        pos += 1; // include the closing ']'

        let value = &text[..pos];
        let post_blank = text[pos..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total_consumed = pos + post_blank;
        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::StatisticsCookie(StatisticsCookieData { value }),
                (start, start + total_consumed),
                self.bump,
            )
            .build(),
        );
        Some((node, total_consumed))
    }

    pub(super) fn try_parse_footnote_reference(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        if !text.starts_with("[fn:") {
            return None;
        }
        let close = {
            let mut depth = 0u32;
            let mut found = None;
            for (i, &b) in text.as_bytes().iter().enumerate() {
                match b {
                    b'[' => depth += 1,
                    b']' => {
                        if depth == 1 {
                            found = Some(i);
                            break;
                        }
                        depth = depth.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            found?
        };
        let inner = &text[4..close];

        let colon_pos = inner.find(':');
        let type_s = colon_pos.map_or("standard", |_| "inline");
        let label = colon_pos.map_or_else(
            || (!inner.is_empty()).then_some(inner),
            |pos| (!inner[..pos].is_empty()).then_some(&inner[..pos]),
        );
        let definition_location = colon_pos.map(|pos| Interval {
            start: start + 4 + pos + 1,
            end: start + close,
        });

        let children = definition_location
            .map(|loc| self.parse_objects(loc, |that| SyntaxT::FootnoteReference.can_contain(that)))
            .unwrap_or_else(|| BumpVec::new_in(self.bump));

        let consumed = close + 1;
        let post_blank = text[consumed..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total_consumed = consumed + post_blank;
        let mut builder = SyntaxNode::new(
            Syntax::FootnoteReference(self.bump.alloc(FootnoteReferenceData { label, type_s })),
            (start, start + total_consumed),
            self.bump,
        );
        if let Some(loc) = definition_location {
            builder = builder.content(loc);
        }
        builder = builder.post_blank(post_blank);
        let node = self.arena.alloc_with_children(builder.build(), children);
        Some((node, total_consumed))
    }
}
