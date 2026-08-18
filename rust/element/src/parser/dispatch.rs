use memchr::{memchr, memrchr};

use super::{ParseGranularity, Parser, ParserMode};
use crate::{
    affiliated::ElementSpan,
    data::NodeId,
    directive::HashDirective,
    environment,
    latex::REGEX_LATEX_BEGIN_ENVIRIONMENT,
    list::{starts_with_item, ListStruct},
    markup::{is_horizontal_rule, REGEX_DIARY_SEXP, REGEX_FOOTNOTE_DEFINITION},
};

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    /// Parse the element starting at cursor position (point).
    #[inline(never)]
    pub fn current_element(
        &mut self,
        limit: usize,
        mode: ParserMode,
        structure: Option<&'b ListStruct<'a, 'b>>,
    ) -> NodeId {
        let pos = self.cursor.pos();

        let raw_secondary_p = self.granularity == ParseGranularity::Object;

        let mut get_current_element = || -> NodeId {
            use crate::parser::ParserMode::*;

            if mode == Item {
                return self.item_parser();
            }

            if mode == TableRow {
                return self.table_row_parser();
            }

            if mode == NodeProperty {
                return self.node_property_parser(limit);
            }

            // Real headlines are parsed structurally here. Inline tasks
            // (15+ stars) are section *content*, so when we are about to open
            // a section we fall through and let `section_parser` wrap them.
            if self.cursor.on_headline()
                && !(matches!(mode, Section | FirstSection) && self.cursor.on_inline_task())
            {
                return self.headline_parser(limit);
            }

            if mode == Section || mode == FirstSection {
                let p = self.cursor.pos();
                let lim = self.cursor.next_real_headline(limit).unwrap_or(limit);
                self.cursor.set(p);
                return self.section_parser(lim);
            }

            {
                let cur = self.cursor.pos();
                let bytes = self.input.as_bytes();
                let line_start = memrchr(b'\n', &bytes[..cur]).map_or(0, |i| i + 1);
                let prev_line_start = if line_start == 0 {
                    0
                } else {
                    memrchr(b'\n', &bytes[..line_start - 1]).map_or(0, |i| i + 1)
                };
                let is_prev_line_headline = bytes.get(prev_line_start) == Some(&b'*');
                let line_limit = memchr(b'\n', &bytes[cur..]).map_or(bytes.len(), |i| cur + i);
                let line = &bytes[cur..line_limit];
                let non_ws = line.iter().position(|&b| b != b' ' && b != b'\t');
                let is_match_planning = non_ws.is_some_and(|i| {
                    let kw = &line[i..];
                    kw.starts_with(b"CLOSED:")
                        || kw.starts_with(b"DEADLINE:")
                        || kw.starts_with(b"SCHEDULED:")
                });

                if mode == Planning && is_prev_line_headline && is_match_planning {
                    return self.planning_parser(limit);
                }
            }

            if !self.cursor.is_bol() {
                let p = self.cursor.pos();
                return self.paragraph_parser(ElementSpan::new((p, limit)).build());
            }

            // --- First-byte dispatch ---
            let cur = self.cursor.pos();
            let b = self.input.as_bytes().get(cur).copied();
            if b.is_none() || b == Some(b'\n') {
                return self.paragraph_parser(ElementSpan::new((cur, limit)).build());
            }

            // Quick dispatches for bytes that uniquely identify an element type.
            // These are simple: one regex match → return.
            if b == Some(b'\\') {
                let span = ElementSpan::new((cur, limit)).build();
                return if self
                    .cursor
                    .looking_at(&*REGEX_LATEX_BEGIN_ENVIRIONMENT)
                    .is_some()
                {
                    self.latex_environment_parser(span)
                } else {
                    self.paragraph_parser(span)
                };
            }
            if b == Some(b'%') {
                let span = ElementSpan::new((cur, limit)).build();
                return if self.cursor.looking_at(&*REGEX_DIARY_SEXP).is_some() {
                    self.diary_sexp_parser(span)
                } else {
                    self.paragraph_parser(span)
                };
            }
            if b == Some(b'[') {
                let span = ElementSpan::new((cur, limit)).build();
                return if mode != FootnoteDefinition
                    && self
                        .cursor
                        .looking_at(&*REGEX_FOOTNOTE_DEFINITION)
                        .is_some()
                {
                    self.footnote_definition_parser(span)
                } else {
                    self.paragraph_parser(span)
                };
            }
            if b == Some(b'*') && self.cursor.on_headline() {
                return self.inlinetask_parser(limit, raw_secondary_p);
            }

            // Non-unique / multi-regex bytes: first collect the element span,
            // then check candidate element types.
            let first_nonws_is_hash = b == Some(b'#')
                || matches!(b, Some(b' ' | b'\t')) && {
                    let rest = &self.input.as_bytes()[cur..];
                    rest.iter()
                        .position(|&c| c != b' ' && c != b'\t')
                        .and_then(|i| rest.get(i))
                        == Some(&b'#')
                };
            let span = if first_nonws_is_hash {
                self.collect_affiliated_keywords(limit)
            } else {
                ElementSpan::new((cur, limit)).build()
            };

            // If affiliated keywords were consumed and we're past the limit,
            // they're orphaned → standalone keyword.
            if span.affiliated.is_some() && self.cursor.pos() >= limit {
                self.cursor.set(span.span.start);
                return self.keyword_parser(ElementSpan::new((span.span.start, limit)).build());
            }

            // Re-check the byte at the current cursor position
            // (may have changed after collect_affiliated_keywords).
            let cur2 = self.cursor.pos();
            let b2 = self.input.as_bytes().get(cur2).copied();

            // For lines with leading whitespace, find the first
            // non-whitespace byte for dispatch.
            let content_byte = match b2 {
                Some(b' ' | b'\t') => {
                    let rest = &self.input.as_bytes()[cur2..];
                    rest.iter()
                        .position(|&c| c != b' ' && c != b'\t')
                        .and_then(|i| rest.get(i).copied())
                }
                other => other,
            };

            match content_byte {
                // Hashtag group: comments, blocks, babel, dynamic, keyword
                Some(b'#') => {
                    // content_byte == b'#' guarantees a '#' exists after optional
                    // leading whitespace at cur2.  Find it with a byte scan instead
                    // of running the regex engine.
                    let bytes = &self.input.as_bytes()[cur2..];
                    let hash_i = bytes
                        .iter()
                        .position(|&b| b != b' ' && b != b'\t')
                        .unwrap_or(0);
                    let end = hash_i + 1; // one past '#'
                    self.cursor.set(cur2 + end);
                    // COLON_OR_EOL: byte after '#' is ' ', '\n', or end-of-input.
                    if matches!(
                        self.input.as_bytes().get(cur2 + end),
                        None | Some(b' ' | b'\n')
                    ) {
                        self.cursor.goto_line_begin();
                        return self.comment_parser(span);
                    }
                    let directive = HashDirective::from(&self.input[self.cursor.pos()..]);
                    self.cursor.goto_line_begin();
                    return match directive {
                        HashDirective::Block(name) => {
                            let mut buf = [0u8; 16];
                            let bytes = name.as_bytes();
                            let len = bytes.len().min(buf.len());
                            for (dst, &src) in buf[..len].iter_mut().zip(&bytes[..len]) {
                                *dst = src.to_ascii_lowercase();
                            }
                            match &buf[..len] {
                                b"center" => self.center_block_parser(span),
                                b"comment" => self.comment_block_parser(span),
                                b"example" => self.example_block_parser(span),
                                b"export" => self.export_block_parser(span),
                                b"quote" => self.quote_block_parser(span),
                                b"src" => self.src_block_parser(span),
                                b"verse" => self.verse_block_parser(span),
                                _ => self.special_block_parser(span),
                            }
                        }
                        HashDirective::BabelCall => self.babel_call_parser(span),
                        HashDirective::DynamicBlock => self.dynamic_block_parser(span),
                        HashDirective::Keyword => self.keyword_parser(span),
                        HashDirective::Unknown => self.paragraph_parser(span),
                    };
                }
                Some(b':') => {
                    let input = self.input.as_bytes();
                    let line_limit =
                        memchr(b'\n', &input[cur2..]).map_or(input.len(), |i| cur2 + i);
                    let line = &input[cur2..line_limit];
                    let colon_pos = line
                        .iter()
                        .position(|&b| b != b' ' && b != b'\t')
                        .unwrap_or(line.len());
                    // Drawer: `:NAME:` followed by optional whitespace
                    if colon_pos < line.len() && line[colon_pos] == b':' {
                        let name_start = colon_pos + 1;
                        let name_end = name_start
                            + line[name_start..]
                                .iter()
                                .position(|&b| {
                                    !(b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                                })
                                .unwrap_or(line.len().saturating_sub(name_start));
                        if name_end > name_start
                            && line.get(name_end) == Some(&b':')
                            && line[name_end + 1..]
                                .iter()
                                .all(|&b| b == b' ' || b == b'\t')
                        {
                            return self.drawer_parser(span, mode);
                        }
                    }
                    // Fixed-width: `: ` or `:` at end-of-line
                    if matches!(line.get(colon_pos + 1), None | Some(b' ' | b'\n')) {
                        return self.fixed_width_parser(span);
                    }
                }
                Some(b'|') => return self.table_parser(span),
                // Horizontal rule
                Some(b'-') => {
                    let line = &self.input[cur2..];
                    let line_end = memchr(b'\n', line.as_bytes()).unwrap_or(line.len());
                    if is_horizontal_rule(&line[..line_end]) {
                        return self.horizontal_rule_parser(span);
                    }
                }
                Some(b'C') => {
                    let input = self.input.as_bytes();
                    let line_limit =
                        memchr(b'\n', &input[cur2..]).map_or(input.len(), |i| cur2 + i);
                    let line = &input[cur2..line_limit];
                    let ws = line
                        .iter()
                        .position(|&b| b != b' ' && b != b'\t')
                        .unwrap_or(line.len());
                    if line[ws..].len() >= 6 && line[ws..][..6].eq_ignore_ascii_case(b"clock:") {
                        return self.clock_line_parser(limit);
                    }
                }
                _ => {}
            }

            // Item (handles leading whitespace via its own check)
            let line = &self.input[cur2..];
            let line_end = memchr(b'\n', line.as_bytes()).unwrap_or(line.len());
            if starts_with_item(&line[..line_end]) {
                let s = structure.unwrap_or_else(|| self.list_struct(limit));
                return self.plain_list_parser(span, s);
            }

            self.paragraph_parser(span)
        };

        let current_element = get_current_element();
        self.cursor.set(pos);
        current_element
    }
}
