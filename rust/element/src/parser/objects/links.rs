use memchr::{memchr, memchr2, memmem};

use super::super::Parser;
use crate::{
    data::{
        BumpVec, Interval, LinkData, LinkFlags, LinkFormat, LinkType, NodeId, RadioTargetData,
        Syntax, SyntaxNode, SyntaxT,
    },
    environment,
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    pub(super) fn try_parse_link(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 4 || &bytes[0..2] != b"[[" {
            return None;
        }

        // Find closing ]], tracking balanced brackets in the description.
        let mut found_close = None;
        let search_pos = 2;
        // Phase 1: path portion — no ] allowed.
        match memchr(b']', &bytes[search_pos..]) {
            Some(offset) => {
                let i = search_pos + offset;
                if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                    // Simple link [[path]] — no description.
                    found_close = Some(i + 2);
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                    // ][ separates path from description.
                    // Phase 2: scan description with balanced bracket tracking.
                    let mut pos = i + 2; // after ][
                    let mut depth = 0i32;
                    while pos + 1 < bytes.len() {
                        match memchr2(b'[', b']', &bytes[pos..]) {
                            Some(offset) => {
                                let j = pos + offset;
                                if bytes[j] == b'[' {
                                    depth += 1;
                                    pos = j + 1;
                                } else {
                                    // bytes[j] == b']'
                                    if j + 1 < bytes.len() && bytes[j + 1] == b']' {
                                        if depth == 0 {
                                            found_close = Some(j + 2);
                                            break;
                                        }
                                        depth -= 1;
                                        pos = j + 2;
                                    } else if j + 1 < bytes.len() && bytes[j + 1] == b'[' {
                                        depth -= 1;
                                        if depth < 0 {
                                            return None;
                                        }
                                        depth += 1; // net 0, skip ][
                                        pos = j + 2;
                                    } else {
                                        depth -= 1;
                                        if depth < 0 {
                                            return None;
                                        }
                                        pos = j + 1;
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
            None => {}
        }

        let close = found_close?;

        // Absorb trailing spaces/tabs into the link's extent, matching Emacs
        // org-element's :post-blank behaviour.  Without this, scan_plain_text_end
        // splits the space off as a lone PlainText before '/' triggers the
        // pre-char stop, producing a node Emacs never emits.
        let post_blank = bytes[close..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let raw = &text[..close];

        // Detect the optional description: the ][ separator between target and
        // description within [[target][description]].  The description is in
        // org-element-contents (unlike the headline title), so it is parsed
        // into child objects.
        let inner = &text[2..close - 2];
        let desc_loc = memmem::find(inner.as_bytes(), b"][").map(|sep| Interval {
            start: start + 2 + sep + 2,
            end: start + close - 2,
        });

        let link_data = LinkData::new(raw);

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Link(self.bump.alloc(link_data)),
                (start, start + close + post_blank),
                self.bump,
            )
            .content((start + 2, start + close - 2))
            .build(),
        );

        if let Some(desc) = desc_loc {
            if desc.start < desc.end {
                let children = self.parse_objects(desc, |that| SyntaxT::Link.can_contain(that));
                self.arena.set_children(node, children);
            }
        }

        Some((node, close + post_blank))
    }

    pub(super) fn try_parse_radio_target(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 6 || &bytes[0..3] != b"<<<" {
            return None;
        }

        // Find closing >>>
        let mut found_close = None;
        let mut search_pos = 3;
        while search_pos + 2 < bytes.len() {
            match memchr(b'>', &bytes[search_pos..]) {
                Some(offset) => {
                    let i = search_pos + offset;
                    if i + 2 < bytes.len() && bytes[i + 1] == b'>' && bytes[i + 2] == b'>' {
                        let after = i + 3;
                        let valid_post =
                            after >= bytes.len() || super::super::is_post_char(bytes[after]);
                        if valid_post {
                            found_close = Some(after);
                            break;
                        }
                        search_pos = i + 3;
                    } else {
                        search_pos = i + 1;
                    }
                }
                None => break,
            }
        }

        let close = found_close?;
        let content = &text[3..close - 3];

        // Absorb trailing spaces/tabs as post-blank, matching Emacs.
        let post_blank = bytes[close..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::RadioTarget(RadioTargetData { raw_value: content }),
                (start, start + close + post_blank),
                self.bump,
            )
            .content((start + 3, start + close - 3))
            .post_blank(post_blank)
            .build(),
        );

        Some((node, close + post_blank))
    }

    pub(super) fn try_parse_radio_link(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        if self.radio_targets.is_empty() {
            return None;
        }

        // Word-boundary check: the character before the match must be
        // non-alphanumeric (or we're at the start of the buffer).
        if start > 0 {
            let prev = self.input.as_bytes()[start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                return None;
            }
        }

        for target in &self.radio_targets {
            if let Some(match_end) = Self::match_radio_target_text(text, target) {
                // Word-boundary after: the byte at `match_end` must be
                // non-alphanumeric or end of buffer (matching Emacs'
                // `[^[:alnum:]]` from `after-re`).  Emacs checks the
                // byte AFTER the captured target text, before any
                // post-blank is consumed.
                if match_end < text.len() {
                    let next = text.as_bytes()[match_end];
                    if next.is_ascii_alphanumeric() || next == b'_' {
                        continue;
                    }
                }

                // Compute post-blank: trailing spaces/tabs after the match
                // (matching Emacs' `skip-chars-forward " \t"`).
                let post_blank = text[match_end..]
                    .bytes()
                    .take_while(|&b| b == b' ' || b == b'\t')
                    .count();
                let consumed = match_end + post_blank;

                let raw = &text[..match_end];
                let link_data = LinkData {
                    flags: LinkFlags::new(LinkFormat::Plain, LinkType::Radio),
                    path: target,
                    raw_link: raw,
                };

                // Radio links in Emacs store the matched text as inner
                // contents (a plain_text child).  Match Emacs by creating
                // the plain text child here.
                let mut inner_children = BumpVec::new_in(self.bump);
                // The content is the matched text WITHOUT post-blank.
                let pt = self.arena.alloc(
                    SyntaxNode::new(
                        Syntax::PlainText(&text[..match_end]),
                        (start, start + match_end),
                        self.bump,
                    )
                    .build(),
                );
                inner_children.push(pt);

                let node = self.arena.alloc_with_children(
                    SyntaxNode::new(
                        Syntax::Link(self.bump.alloc(link_data)),
                        (start, start + consumed),
                        self.bump,
                    )
                    .content((start, start + consumed))
                    .build(),
                    inner_children,
                );

                return Some((node, consumed));
            }
        }

        None
    }

    pub(super) fn try_parse_target(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 5 || &bytes[0..2] != b"<<" {
            return None;
        }

        // Emacs org-target-regexp:
        //   <<\([^<>\n \t]\|[^<>\n \t][^<>\n]*[^<>\n \t]\)>>
        // The content must be non-empty, contain no `<`, `>` or newline, and
        // must not start or end with a space/tab.  There is no post-char
        // restriction after the closing `>>`.
        let mut i = 2;
        let close = loop {
            if i >= bytes.len() {
                return None;
            }
            match bytes[i] {
                // `<` or newline inside the content invalidate the target.
                b'<' | b'\n' => return None,
                b'>' => {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                        break i + 2; // position just after the closing `>>`
                    }
                    // A lone `>` cannot appear in target content.
                    return None;
                }
                _ => i += 1,
            }
        };

        let content = &text[2..close - 2];
        let cb = content.as_bytes();
        if cb.is_empty()
            || matches!(cb[0], b' ' | b'\t')
            || matches!(cb[cb.len() - 1], b' ' | b'\t')
        {
            return None;
        }

        // Absorb trailing spaces/tabs as post-blank, matching org-element.
        let post_blank = bytes[close..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Target(content),
                (start, start + close + post_blank),
                self.bump,
            )
            .post_blank(post_blank)
            .build(),
        );

        Some((node, close + post_blank))
    }

    pub(super) fn try_parse_plain_link(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        let proto_len = self.proto_len(bytes)?;
        let url_end_raw = text[proto_len..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '[' | ']' | '<' | '>' | '(' | ')' | '\'' | '"')
            })
            .map_or(text.len(), |i| proto_len + i);
        if url_end_raw <= proto_len {
            return None;
        }
        // Trim trailing punctuation that Emacs excludes from plain links.
        // Characters like `.` and `:` are valid URL-internal but not as the
        // last character in a plain link.  Walking backwards from url_end_raw
        // strips them so the link ends before e.g. `https://x.com.` → `https://x.com`.
        let url_end = text[proto_len..url_end_raw]
            .char_indices()
            .rev()
            .find(|&(_, c)| {
                !matches!(
                    c,
                    '.' | ',' | ':' | ';' | '!' | '?' | ')' |
                    '\u{00AB}' | '\u{00BB}' |
                    // CJK full stops / commas that Emacs also excludes
                    '\u{3001}' | '\u{3002}' |
                    '\u{FF0C}' | '\u{FF0E}' | '\u{FF01}' | '\u{FF1F}'
                )
            })
            .map_or(proto_len, |(i, c)| proto_len + i + c.len_utf8());
        if url_end <= proto_len {
            return None;
        }
        let raw = &text[..url_end];
        let post_blank = text[url_end..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let consumed = url_end + post_blank;
        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Link(self.bump.alloc(LinkData::new_plain(raw))),
                (start, start + consumed),
                self.bump,
            )
            .build(),
        );
        Some((node, consumed))
    }

    pub(super) fn try_parse_angle_link(
        &mut self,
        text: &'a str,
        start: usize,
    ) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.first() != Some(&b'<') {
            return None;
        }
        // Closing `>` must exist within the remaining text
        let close = memchr(b'>', &bytes[1..]).map(|i| i + 1)?;
        if close < 2 {
            return None;
        }
        // Inner content must start with a recognized protocol
        let inner = &text[1..close];
        const PROTOCOLS: &[&[u8]] = &[
            b"https://",
            b"http://",
            b"ftp://",
            b"mailto:",
            b"id:",
            b"file:",
        ];
        if !PROTOCOLS.iter().any(|p| inner.as_bytes().starts_with(p)) {
            return None;
        }
        let raw = &text[..close + 1];
        let post_blank = text[close + 1..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let consumed = close + 1 + post_blank;
        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Link(self.bump.alloc(LinkData::new_angle(raw))),
                (start, start + consumed),
                self.bump,
            )
            .build(),
        );
        Some((node, consumed))
    }
}
