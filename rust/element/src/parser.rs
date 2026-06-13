//    This file is part of org-rs.
//
//    org-rs is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    org-rs is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with org-rs.  If not, see <https://www.gnu.org/licenses/>.
#![allow(
    clippy::needless_range_loop,
    reason = "This is a moronic lint, that makes simple interactions complicated for no reason"
)]
use memchr::{memchr, memchr2, memchr3, memmem};

use crate::{
    affiliated::ElementSpan,
    babel::REGEX_BABEL_CALL,
    blocks::{REGEX_BLOCK_BEGIN, REGEX_DYNAMIC_BLOCK},
    cursor::Cursor,
    data::{
        Brackets, BumpVec, CitationData, EntityData, FootnoteReferenceData, InlineBabelCallData,
        Interval, LinkData, LinkFlags, LinkFormat, LinkType, MacroData, NodeArena, NodeId,
        RadioTargetData, ScriptFlags, ScriptKind, StatisticsCookieData, Syntax, SyntaxNode,
        SyntaxT, TimestampData,
    },
    drawer::REGEX_DRAWER,
    environment,
    headline::{REGEX_CLOCK_LINE, REGEX_PLANNING_LINE},
    keyword::*,
    latex::REGEX_LATEX_BEGIN_ENVIRIONMENT,
    list::*,
    markup::{REGEX_DIARY_SEXP, REGEX_FOOTNOTE_DEFINITION},
    table::REGEX_TABLE_BORDER,
};

/// determines the depth of the recursion.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ParseGranularity {
    /// Only parse headlines.
    Headline,
    /// Don't recurse into greater elements except headlines and
    /// sections.  Thus, elements parsed are the top-level ones.
    GreaterElement,
    /// Parse everything but objects and plain text.
    Element,
    /// Parse the complete buffer.
    #[default]
    Object,
}

/// MODE prioritizes some elements over the others
///
/// @ngortheone - it looks like these are states of parser's finite automata
#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum ParserMode {
    Nil,
    FirstSection,
    Section,
    Planning,
    Item,
    NodeProperty,
    TableRow,
    PropertyDrawer,
}

pub struct Parser<'input, 'bumpalo, Environment: environment::Environment> {
    pub cursor: Cursor<'input>,
    pub input: &'input str,
    pub granularity: ParseGranularity,
    pub environment: Environment,
    pub arena: NodeArena<'input, 'bumpalo>,
    pub bump: &'bumpalo bumpalo::Bump,
    pub object_region_start: usize,
    /// Tab width for `string-width`-style indent calculations.
    /// Defaults to 8 (Emacs standard).  Unlike screen-column math,
    /// a tab ALWAYS counts as `tab_width` columns regardless of
    /// the current column position.
    pub tab_width: u8,
    /// Indent of the enclosing list item when `list_struct` is called while
    /// scanning that item's content.  `None` at top-level (section/headline).
    pub item_indent_ctx: Option<usize>,
    /// Radio target texts collected from `<<<...>>>` definitions.
    /// Used to detect radio links during object parsing.
    radio_targets: Vec<&'input str>,
}

macro_rules! looking_at {
    ($regex:ident, $parser: ident) => {
        $parser.cursor.looking_at(&*$regex)
    };
}

macro_rules! capturing_at {
    ($regex:ident, $parser: ident) => {
        $parser.cursor.capturing_at(&*$regex)
    };
}

/// Non-breaking space (U+00A0) — Emacs syntax-table classifies this as
/// whitespace, so it must not appear at an emphasis content boundary.
const NBSP: char = '\u{00a0}';
/// Zero-width space (U+200B) — Emacs syntax-table classifies this as
/// whitespace, so it must not appear at an emphasis content boundary.
const ZWS: char = '\u{200b}';

/// Returns `true` when `b` is an Org-mode pre-character: a byte that may
/// immediately precede an emphasis marker or plain-link protocol to open a
/// markup span.
#[inline]
fn is_pre_char(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'(' | b'{' | b'\'' | b'"' | b'-')
}

/// Returns `true` when `b` is an Org-mode post-character: a byte that may
/// immediately follow a closing emphasis marker or the end of a plain link.
#[inline]
fn is_post_char(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t'
            | b'\n'
            | b'.'
            | b','
            | b'!'
            | b'?'
            | b';'
            | b':'
            | b'\''
            | b'"'
            | b')'
            | b'}'
            | b'\\'
            | b'['
            | b'-'
    )
}

/// If `bytes` begins with one of `link_types` followed by `:`, return the
/// length of that `type:` prefix. Used to recognise plain links; the type set
/// is supplied by the [`Environment`](crate::environment::Environment).
#[inline]
fn plain_link_proto_len(bytes: &[u8], link_types: &[&str]) -> Option<usize> {
    link_types.iter().find_map(|ty| {
        let t = ty.as_bytes();
        (bytes.starts_with(t) && bytes.get(t.len()) == Some(&b':')).then_some(t.len() + 1)
    })
}

/// Match `text` against a radio target string, with case-insensitive
/// comparison and whitespace-run collapsing matching Emacs' `\\s-+`
/// semantics (newlines, tabs, and spaces are all treated equally).
/// Returns the number of bytes consumed from `text` on success, or
/// `None` if no match.
/// Pre-scan `input` for `<<<...>>>` radio target definitions and
/// collect the target texts.  These are used later to detect radio
/// links during object parsing.  Emacs builds a single compound
/// regex from all targets (see `ol.el`'s `org-update-radio-target-regexp`),
/// applying case-insensitive matching and collapsing whitespace runs
/// (spaces, newlines, tabs) so `<<<Special comment section>>>` matches
/// `"special comment\n  section"` in the document body.
///
/// Scanning is line-based, matching Emacs' `org-radio-target-regexp` which
/// does not span line boundaries.  We also skip matches that appear inside
/// `=...=` or `~...~` inline code spans.
fn collect_radio_targets(input: &str) -> Vec<&str> {
    let mut targets: Vec<&str> = Vec::new();
    for line in input.lines() {
        if let Some(target) = extract_radio_target_line(line) {
            if !target.is_empty() && !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

/// Scan a single line for a `<<<...>>>` radio target, skipping inline
/// code spans delimited by `=` or `~`.
fn extract_radio_target_line(line: &str) -> Option<&str> {
    let b = line.as_bytes();
    // Quick check: must contain <<< and >>>
    let open = memmem::find(b, b"<<<")?;
    let after_open = open + 3;
    let close = memmem::find(&b[after_open..], b">>>")?;
    let target = &line[after_open..after_open + close];

    // Skip if the match is inside =...= or ~...~ inline code.
    // We check whether the text between `=...=` or `~...~` on this line
    // fully contains the <<<...>>> range.
    if is_inside_code_span(line, open, after_open + close + 3) {
        return None;
    }

    Some(target)
}

/// Returns true if the byte range [start..end) on `line` falls entirely
/// inside a `=...=` or `~...~` inline code span.
fn is_inside_code_span(line: &str, start: usize, _end: usize) -> bool {
    let b = line.as_bytes();
    // Find all = and ~ positions (as potential code span delimiters)
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' || b[i] == b'~' {
            // Check if this is a code delimiter (preceded by space, tab, start-of-line,
            // or punctuation; followed by a non-space)
            if i == 0 || b[i - 1].is_ascii_whitespace() || is_pre_char(b[i - 1]) {
                // Find matching close delimiter on same line
                if let Some(close) = memmem::find(&b[i + 1..], &[b[i]]) {
                    let close_pos = i + 1 + close;
                    // Make sure close is followed by boundary
                    if close_pos + 1 >= b.len()
                        || b[close_pos + 1].is_ascii_whitespace()
                        || is_post_char(b[close_pos + 1])
                    {
                        // Range [i..=close_pos] is a code span
                        if start >= i && start <= close_pos {
                            return true;
                        }
                        i = close_pos + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

fn match_radio_target_text(text: &str, target: &str) -> Option<usize> {
    let t = target.as_bytes();
    let s = text.as_bytes();
    let (mut ti, mut si) = (0, 0);

    while ti < t.len() && si < s.len() {
        let tc = t[ti];
        let sc = s[si];

        if tc == b' ' {
            // Target has whitespace — consume one or more whitespace bytes
            // in source (matching Emacs' `\\s-+`), AND skip over all
            // consecutive spaces in the target so that multiple spaces
            // in the definition (e.g. "<<<Special   comment>>>") still
            // match a single space in the running text.
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
        } else {
            // Non-space character: case-insensitive compare
            if tc.eq_ignore_ascii_case(&sc) {
                ti += 1;
                si += 1;
            } else if sc.is_ascii_whitespace() {
                // Source has unexpected whitespace — skip over it
                // (handles cases where Emacs' `\\s-+` matches extra
                // whitespace not present in the target).
                si += 1;
            } else {
                return None;
            }
        }
    }

    if ti == t.len() {
        // Match succeeded — do NOT consume trailing whitespace.
        // Emacs separates the match extent (link-end / match-end 1)
        // from post-blank, so trailing space/tab are handled by the
        // caller.
        Some(si)
    } else {
        None
    }
}

/// Scan `bytes` for the first position that ends a plain-text run,
/// returning the number of bytes that belong to the run.  Returns
/// zero when no plain-text bytes are available at the start of
/// `bytes`.
///
/// `link_types` / `link_start_bytes` configure plain-link recognition: the
/// scan pauses at every `link_start_bytes` byte and confirms a real link via
/// `link_types`.
#[inline]
fn scan_plain_text_end(bytes: &[u8], link_types: &[&str], link_start_bytes: &[u8]) -> usize {
    // Markup delimiters are fixed, so keep SIMD `memchr` for them. Plain-link
    // start bytes are configurable, so locate them with a membership table.
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
        if matches!(b, b'*' | b'/' | b'+' | b'=' | b'~') && i > 0 && is_pre_char(bytes[i - 1]) {
            return i;
        }
        if matches!(b, b'c') && i > 0 && is_pre_char(bytes[i - 1]) {
            return i;
        }
        if matches!(b, b'_' | b'^') && i > 0 {
            return i;
        }
        // A plain link must sit at a word boundary and start a known type.
        // Accept any non-ASCII byte as a pre-char (CJK, etc.) — Emacs'
        // plain-link regex does not require a word boundary at all.
        // Unlike emphasis markers, `'` is NOT a valid pre-char for plain
        // links — Emacs does not recognise `'file:…'` as a link.
        if (i == 0 || (is_pre_char(bytes[i - 1]) && bytes[i - 1] != b'\'') || bytes[i - 1] >= 0x80)
            && plain_link_proto_len(&bytes[i..], link_types).is_some()
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
            // Any other paused byte is a (possibly non-link) link-start byte.
            _ => pl = find_link(rest).map(|r| next + r),
        }
    }
}

impl<'a, 'b, Environment: environment::Environment> Parser<'a, 'b, Environment> {
    #[inline]
    pub fn new(
        input: &'a str,
        granularity: ParseGranularity,
        environment: Environment,
        bump: &'b bumpalo::Bump,
    ) -> Parser<'a, 'b, Environment> {
        let tab_width = environment.tab_width();
        let radio_targets = collect_radio_targets(input);
        Parser {
            cursor: Cursor::new(input, 0),
            input,
            granularity,
            environment,
            arena: NodeArena::new(),
            bump,
            object_region_start: 0,
            tab_width,
            item_indent_ctx: None,
            radio_targets,
        }
    }

    /// Returns parser mode according to given `element` and `is_parent`
    /// `element` is AllElements variant representing the type of an element
    /// containing next element if `is_parent` is true, or before it
    /// otherwise.
    /// <br>
    /// Original function name: org-element--next-mode
    /// https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L4273
    fn next_mode(mode: ParserMode, syntax: SyntaxT, is_parent: bool) -> ParserMode {
        use ParserMode::Planning as PmPlanning;
        use ParserMode::PropertyDrawer as PmPropertyDrawer;
        use SyntaxT::*;

        if is_parent {
            match syntax {
                Headline => ParserMode::Section,
                InlineTask => ParserMode::Planning,
                Item => ParserMode::Planning,
                PlainList => ParserMode::Item,
                PropertyDrawer => ParserMode::NodeProperty,
                Section => ParserMode::Planning,
                Table => ParserMode::TableRow,
                _ => mode,
            }
        } else {
            use ParserMode::FirstSection as PmFirstSection;
            use ParserMode::Nil as PmNil;
            match (mode, syntax) {
                // Planning mode: stay in planning only for planning elements,
                // otherwise reset to a neutral mode (Emacs returns nil here,
                // which disables the property-drawer shortcut).
                (PmPlanning, Planning) => PmPropertyDrawer,
                (PmPlanning, _) => PmNil,
                // top-comment / first-section: after a comment, expect a
                // property drawer (Emacs: `top-comment + comment → property-drawer`).
                (PmFirstSection, Comment) => PmPropertyDrawer,
                _ => match syntax {
                    Item => ParserMode::Item,
                    NodeProperty => ParserMode::NodeProperty,
                    Planning => PmPropertyDrawer,
                    TableRow => ParserMode::TableRow,
                    _ => mode,
                },
            }
        }
    }

    /// org-element-parse-buffer
    /// Parses input from beginning to the end
    #[inline]
    pub fn parse_buffer(&mut self) -> (NodeArena<'a, 'b>, NodeId) {
        self.cursor.set(0);
        self.cursor.skip_whitespace();

        let end = self.input.len();
        // Pre-size the arena to avoid reallocation copies: ~1 node per 100
        // bytes of input, capped at 4096 to keep the working set in L2 cache.
        if end > 0 {
            self.arena.nodes.reserve((end / 100).min(4096));
        }
        let root = self.arena.alloc(SyntaxNode::create_root(self.bump));
        let children = self.parse_elements((0, end), ParserMode::FirstSection, None);
        self.arena.set_children(root, children);

        (std::mem::take(&mut self.arena), root)
    }

    /// Parse elements between BEG and END positions.
    #[inline(never)]
    pub fn parse_elements(
        &mut self,
        span: impl Into<Interval>,
        mut mode: ParserMode,
        structure: Option<&'b ListStruct<'a, 'b>>,
    ) -> BumpVec<'b, NodeId> {
        let span = span.into();
        let pos = self.cursor.pos();
        self.cursor.set(span.start);

        if self.granularity == ParseGranularity::Headline && !self.cursor.on_headline() {
            self.cursor.next_headline();
        }

        let mut elements: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
        loop {
            let current_pos = self.cursor.pos();
            if current_pos >= span.end {
                break;
            }

            {
                let line_end = memchr(b'\n', &self.input.as_bytes()[current_pos..span.end])
                    .map_or(span.end, |i| current_pos + i + 1);
                if self.input.as_bytes()[current_pos..line_end]
                    .trim_ascii()
                    .is_empty()
                {
                    self.cursor.set(line_end);
                    continue;
                }
            }

            let element = self.current_element(span.end, mode, structure);

            let element_end;
            let element_content;
            {
                let node = self.arena.get(element);
                element_end = node.location.end;
                element_content = node.content_location;
            }
            self.cursor.set(element_end);

            if let Some(content_location) = element_content {
                let is_greater;
                let data_disc;
                {
                    let node = self.arena.get(element);
                    is_greater = SyntaxT::from(&node.data).is_greater_element();
                    data_disc = SyntaxT::from(&node.data);
                }

                if is_greater {
                    let recurse = (SyntaxT::Headline == data_disc)
                        || (self.granularity == ParseGranularity::Element
                            || self.granularity == ParseGranularity::Object)
                        || ((SyntaxT::Section == data_disc)
                            && (self.granularity == ParseGranularity::GreaterElement));

                    if recurse {
                        let list_sturct = {
                            let node = self.arena.get(element);
                            match &node.data {
                                Syntax::PlainList(d) => Some(d.structure),
                                _ => None,
                            }
                        };

                        let new_mode = Parser::<Environment>::next_mode(mode, data_disc, true);

                        let children = self.parse_elements(content_location, new_mode, list_sturct);
                        self.arena.set_children(element, children);
                    }
                } else if let ParseGranularity::Object = &self.granularity {
                    let children =
                        self.parse_objects(content_location, |that| data_disc.can_contain(that));
                    self.arena.set_children(element, children);
                }
            }

            if self.granularity == ParseGranularity::Object {
                let title_location = {
                    let node = self.arena.get(element);
                    if let Syntax::Headline(ref data) = node.data {
                        data.title_location
                    } else {
                        None
                    }
                };
                if let Some(loc) = title_location {
                    let title_objects =
                        self.parse_objects(loc, |that| SyntaxT::Headline.can_contain(that));
                    self.arena.set_title_objects(element, title_objects);
                }
            }

            {
                let node = self.arena.get(element);
                mode = Parser::<Environment>::next_mode(mode, SyntaxT::from(&node.data), false)
            }
            elements.push(element);
        }
        self.cursor.set(pos);
        elements
    }

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
                return self.headline_parser();
            }

            if mode == Section || mode == FirstSection {
                let p = self.cursor.pos();
                let lim = self.cursor.next_real_headline().unwrap_or(limit).min(limit);
                self.cursor.set(p);
                return self.section_parser(lim);
            }

            {
                let maybe_headline_offset = self.cursor.line_beginning_position(Some(0));
                let is_prev_line_headline =
                    self.input.as_bytes().get(maybe_headline_offset) == Some(&b'*');
                let is_match_planning = self.cursor.looking_at(&REGEX_PLANNING_LINE).is_some();

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
                return if looking_at!(REGEX_LATEX_BEGIN_ENVIRIONMENT, self).is_some() {
                    self.latex_environment_parser(span)
                } else {
                    self.paragraph_parser(span)
                };
            }
            if b == Some(b'%') {
                let span = ElementSpan::new((cur, limit)).build();
                return if looking_at!(REGEX_DIARY_SEXP, self).is_some() {
                    self.diary_sexp_parser(span)
                } else {
                    self.paragraph_parser(span)
                };
            }
            if b == Some(b'[') {
                let span = ElementSpan::new((cur, limit)).build();
                return if looking_at!(REGEX_FOOTNOTE_DEFINITION, self).is_some() {
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
                    let block_name = capturing_at!(REGEX_BLOCK_BEGIN, self)
                        .and_then(|cap| cap.get(1).map(|m| m.as_str()));
                    if let Some(name) = block_name {
                        self.cursor.goto_line_begin();
                        let mut buf = [0u8; 16];
                        let bytes = name.as_bytes();
                        let len = bytes.len().min(buf.len());
                        for (dst, &src) in buf[..len].iter_mut().zip(&bytes[..len]) {
                            *dst = src.to_ascii_lowercase();
                        }
                        return match &buf[..len] {
                            b"center" => self.center_block_parser(span),
                            b"comment" => self.comment_block_parser(span),
                            b"example" => self.example_block_parser(span),
                            b"export" => self.export_block_parser(span),
                            b"quote" => self.quote_block_parser(span),
                            b"src" => self.src_block_parser(span),
                            b"verse" => self.verse_block_parser(span),
                            _ => self.special_block_parser(span),
                        };
                    }
                    if looking_at!(REGEX_BABEL_CALL, self).is_some() {
                        self.cursor.goto_line_begin();
                        return self.babel_call_parser(span);
                    }
                    if looking_at!(REGEX_DYNAMIC_BLOCK, self).is_some() {
                        self.cursor.goto_line_begin();
                        return self.dynamic_block_parser(span);
                    }
                    if looking_at!(REGEX_KEYWORD, self).is_some() {
                        self.cursor.goto_line_begin();
                        return self.keyword_parser(span);
                    }
                    self.cursor.goto_line_begin();
                    return self.paragraph_parser(span);
                }
                // Drawer / fixed-width
                Some(b':') => {
                    if looking_at!(REGEX_DRAWER, self).is_some() {
                        return self.drawer_parser(span, mode);
                    }
                    // Fixed-width: `[ \t]*:( |$)` — content_byte is ':' so the colon
                    // position is already known; just test the byte that follows it.
                    {
                        let bytes = &self.input.as_bytes()[cur2..];
                        let colon_i = bytes
                            .iter()
                            .position(|&b| b != b' ' && b != b'\t')
                            .unwrap_or(0);
                        if matches!(bytes.get(colon_i + 1), None | Some(b' ' | b'\n')) {
                            return self.fixed_width_parser(span);
                        }
                    }
                }
                // Table
                #[allow(clippy::collapsible_match)]
                Some(b'|') => {
                    if looking_at!(REGEX_TABLE_BORDER, self).is_some() {
                        return self.table_parser(span);
                    }
                }
                // Horizontal rule
                Some(b'-') => {
                    let line = &self.input[cur2..];
                    let line_end = memchr(b'\n', line.as_bytes()).unwrap_or(line.len());
                    if crate::markup::is_horizontal_rule(&line[..line_end]) {
                        return self.horizontal_rule_parser(span);
                    }
                }
                // Clock (with leading whitespace allowed)
                #[allow(clippy::collapsible_match)]
                Some(b'C') => {
                    if looking_at!(REGEX_CLOCK_LINE, self).is_some() {
                        return self.clock_line_parser(limit);
                    }
                }
                _ => {}
            }

            // Item (handles leading whitespace via its own check)
            let line = &self.input[cur2..];
            let line_end = memchr(b'\n', line.as_bytes()).unwrap_or(line.len());
            if crate::list::starts_with_item(&line[..line_end]) {
                let s = structure.unwrap_or_else(|| self.list_struct(limit));
                return self.plain_list_parser(span, s);
            }

            self.paragraph_parser(span)
        };

        let current_element = get_current_element();
        self.cursor.set(pos);
        current_element
    }

    /// Parse objects between `beg` and `end` and return recursive structure.
    #[inline(never)]
    pub fn parse_objects(
        &mut self,
        interval: impl Into<Interval>,
        restriction: impl Fn(SyntaxT) -> bool,
    ) -> BumpVec<'b, NodeId> {
        let interval = interval.into();
        // Mark this region's start so an emphasis marker sitting at the
        // boundary passes the pre-character check. Saved/restored to keep the
        // enclosing region's boundary intact across nested object parsing.
        let saved_region_start = self.object_region_start;
        self.object_region_start = interval.start;
        let mut children: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
        let mut pos = interval.start;

        while pos < interval.end {
            let remaining = &self.input[pos..interval.end];

            let consumed = {
                let bytes = remaining.as_bytes();
                if bytes.is_empty() {
                    None
                } else {
                    match bytes[0] {
                        b'*' => self.try_parse_bold(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Bold) {
                                children.push(n);
                            }
                            c
                        }),
                        b'/' => self.try_parse_italic(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Italic) {
                                children.push(n);
                            }
                            c
                        }),
                        b'~' => self.try_parse_code(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Code) {
                                children.push(n);
                            }
                            c
                        }),
                        b'=' => self.try_parse_verbatim(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Verbatim) {
                                children.push(n);
                            }
                            c
                        }),
                        b'_' => {
                            if let Some((n, c)) =
                                self.try_parse_script(remaining, pos, ScriptKind::Sub)
                            {
                                if restriction(SyntaxT::Script) {
                                    children.push(n);
                                }
                                Some(c)
                            } else {
                                self.try_parse_underline(remaining, pos).map(|(n, c)| {
                                    if restriction(SyntaxT::Underline) {
                                        children.push(n);
                                    }
                                    c
                                })
                            }
                        }
                        b'^' => {
                            self.try_parse_script(remaining, pos, ScriptKind::Sup)
                                .map(|(n, c)| {
                                    if restriction(SyntaxT::Script) {
                                        children.push(n);
                                    }
                                    c
                                })
                        }
                        b'+' => self.try_parse_strikethrough(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::StrikeThrough) {
                                children.push(n);
                            }
                            c
                        }),
                        b'[' => {
                            let mut result = None;
                            // A statistics cookie is only recognised where it is
                            // permitted (e.g. not inside a table cell); otherwise
                            // `[/]` stays plain text, matching Emacs.
                            if restriction(SyntaxT::StatisticsCookie) {
                                if let Some((n, c)) =
                                    self.try_parse_statistics_cookie(remaining, pos)
                                {
                                    children.push(n);
                                    result = Some(c);
                                }
                            }
                            if result.is_none() {
                                if let Some((n, c)) =
                                    self.try_parse_footnote_reference(remaining, pos)
                                {
                                    if restriction(SyntaxT::FootnoteReference) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                } else if let Some((n, c)) =
                                    self.try_parse_timestamp(remaining, pos)
                                {
                                    if restriction(SyntaxT::Timestamp) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                } else if let Some((n, c)) = self.try_parse_citation(remaining, pos)
                                {
                                    if restriction(SyntaxT::Citation) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                } else if let Some((n, c)) = self.try_parse_link(remaining, pos) {
                                    if restriction(SyntaxT::Link) {
                                        children.push(n);
                                    }
                                    result = Some(c);
                                }
                            }
                            result
                        }
                        b'\\' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_latex_math(remaining, pos) {
                                if restriction(SyntaxT::LatexFragment) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_line_break(remaining, pos) {
                                if restriction(SyntaxT::LineBreak) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) =
                                self.try_parse_latex_fragment(remaining, pos)
                            {
                                if restriction(SyntaxT::LatexFragment) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_entity(remaining, pos) {
                                if restriction(SyntaxT::Entity) {
                                    children.push(n);
                                }
                                result = Some(c);
                            }
                            result
                        }
                        b'$' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_latex_math(remaining, pos) {
                                if restriction(SyntaxT::LatexFragment) {
                                    children.push(n);
                                }
                                result = Some(c);
                            }
                            result
                        }
                        b'<' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_timestamp(remaining, pos) {
                                if restriction(SyntaxT::Timestamp) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_radio_target(remaining, pos)
                            {
                                if restriction(SyntaxT::RadioTarget) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_target(remaining, pos) {
                                if restriction(SyntaxT::Target) {
                                    children.push(n);
                                }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_angle_link(remaining, pos) {
                                if restriction(SyntaxT::Link) {
                                    children.push(n);
                                }
                                result = Some(c);
                            }
                            result
                        }
                        b'{' => self.try_parse_macro(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Macro) {
                                children.push(n);
                            }
                            c
                        }),
                        b'c' => self
                            .try_parse_inline_babel_call(remaining, pos)
                            .map(|(n, c)| {
                                if restriction(SyntaxT::InlineBabelCall) {
                                    children.push(n);
                                }
                                c
                            }),
                        // Plain-link start bytes come from the environment so
                        // the recognised types stay configurable.
                        _ if self.environment.link_start_bytes().contains(&bytes[0]) => {
                            self.try_parse_plain_link(remaining, pos).map(|(n, c)| {
                                if restriction(SyntaxT::Link) {
                                    children.push(n);
                                } else {
                                    // Link not allowed in
                                    // (e.g. another link).  Absorb
                                    // the URL text into the preceding
                                    // PlainText so the node
                                    // boundaries match Emacs.
                                    let fused = children.last().copied().and_then(|prev| {
                                        if let Syntax::PlainText(_) = &self.arena.nodes[prev].data {
                                            if self.arena.nodes[prev].location.end == pos {
                                                let new_start =
                                                    self.arena.nodes[prev].location.start;
                                                return Some((prev, new_start, pos + c));
                                            }
                                        }
                                        None
                                    });
                                    if let Some((prev, new_start, new_end)) = fused {
                                        self.arena.nodes[prev].data =
                                            Syntax::PlainText(&self.input[new_start..new_end]);
                                        self.arena.nodes[prev].location = Interval {
                                            start: new_start,
                                            end: new_end,
                                        };
                                    } else {
                                        let pt = self.arena.alloc(
                                            SyntaxNode::new(
                                                Syntax::PlainText(&self.input[pos..pos + c]),
                                                (pos, pos + c),
                                                self.bump,
                                            )
                                            .build(),
                                        );
                                        children.push(pt);
                                    }
                                }
                                c
                            })
                        }
                        _ => None,
                    }
                }
            };

            if let Some(c) = consumed {
                pos += c;
            } else if let Some((node, c)) = self.try_parse_radio_link(remaining, pos) {
                children.push(node);
                pos += c;
            } else if let Some((node, c)) = self.try_parse_plain_text(remaining, pos) {
                // Coalesce with the previous child if it is also a PlainText
                // ending exactly here.  This fuses the two pieces that result
                // when scan_plain_text_end stops at a markup-potential character
                // ('+', '/', …) whose subsequent markup parse then fails.
                let fused = children.last().copied().and_then(|prev| {
                    if let Syntax::PlainText(_) = &self.arena.nodes[prev].data {
                        if self.arena.nodes[prev].location.end == pos {
                            let new_start = self.arena.nodes[prev].location.start;
                            let new_end = pos + c;
                            return Some((prev, new_start, new_end));
                        }
                    }
                    None
                });
                if let Some((prev, new_start, new_end)) = fused {
                    self.arena.nodes[prev].data =
                        Syntax::PlainText(&self.input[new_start..new_end]);
                    self.arena.nodes[prev].location = Interval {
                        start: new_start,
                        end: new_end,
                    };
                } else {
                    children.push(node);
                }
                pos += c;
            } else {
                // No construct recognised and scan_plain_text_end treats this
                // byte as an unconditional stop (e.g. `[`, `\`, `<`).
                // Extend the previous PlainText by one byte, or start a new one,
                // so the node boundary matches Emacs (which keeps such bytes in
                // the surrounding PlainText rather than creating a gap).
                let fused = children.last().copied().and_then(|prev| {
                    if let Syntax::PlainText(_) = &self.arena.nodes[prev].data {
                        if self.arena.nodes[prev].location.end == pos {
                            let new_start = self.arena.nodes[prev].location.start;
                            return Some((prev, new_start, pos + 1));
                        }
                    }
                    None
                });
                if let Some((prev, new_start, new_end)) = fused {
                    self.arena.nodes[prev].data =
                        Syntax::PlainText(&self.input[new_start..new_end]);
                    self.arena.nodes[prev].location = Interval {
                        start: new_start,
                        end: new_end,
                    };
                } else {
                    let byte_node = self.arena.alloc(
                        SyntaxNode::new(
                            Syntax::PlainText(&self.input[pos..pos + 1]),
                            (pos, pos + 1),
                            self.bump,
                        )
                        .build(),
                    );
                    children.push(byte_node);
                }
                pos += 1;
            }
        }

        self.object_region_start = saved_region_start;
        children
    }

    fn try_parse_bold(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'*', SyntaxT::Bold)
    }

    fn try_parse_italic(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'/', SyntaxT::Italic)
    }

    fn try_parse_underline(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'_', SyntaxT::Underline)
    }

    fn try_parse_strikethrough(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'+', SyntaxT::StrikeThrough)
    }

    fn try_parse_code(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'~', SyntaxT::Code)
    }

    fn try_parse_verbatim(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        self.parse_emphasis_marker(text, start, b'=', SyntaxT::Verbatim)
    }

    fn try_parse_script(
        &mut self,
        text: &'a str,
        start: usize,
        kind: ScriptKind,
    ) -> Option<(NodeId, usize)> {
        // FIXME: This looks hella sus
        let _ = start
            .checked_sub(1)
            .and_then(|i| self.input.as_bytes().get(i))
            .filter(|&&b| !matches!(b, b' ' | b'\t' | b'\n'))?;

        let bytes = text.as_bytes();

        let (brackets, consumed) = match bytes.get(1)? {
            b'{' => {
                let rest = bytes.get(2..)?;
                let close = memchr(b'}', rest)
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

        let plain_text = self.arena.alloc(
            SyntaxNode::new(
                Syntax::PlainText(&self.input[content_begin..content_end]),
                (content_begin, content_end),
                self.bump,
            )
            .build(),
        );

        // Absorb trailing whitespace as post-blank, matching Emacs
        // org-element behaviour for bare sub/superscripts.
        let post_blank = bytes[consumed..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();

        let flags = ScriptFlags::new(kind, brackets);
        let mut script_children: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
        script_children.push(plain_text);
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

    fn try_parse_link(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    fn try_parse_radio_target(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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
                        let valid_post = after >= bytes.len() || is_post_char(bytes[after]);
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

    /// Pre-scan `input` for `<<<...>>>` radio target definitions and
    /// collect the target texts.  These are used later to detect radio
    /// links during object parsing.  Emacs builds a single compound
    /// regex from all targets (see `ol.el`'s `org-update-radio-target-regexp`),
    /// applying case-insensitive matching and collapsing whitespace runs
    /// (spaces, newlines, tabs) so `<<<Special comment section>>>` matches
    /// `"special comment\n  section"` in the document body.
    /// Try to parse a radio link at the current position.
    ///
    /// A radio link occurs when the text matches a previously-defined
    /// radio target (`<<<target>>>`).  Per Emacs' `org-element-link-parser`,
    /// the match is case-insensitive and whitespace runs (including
    /// newlines) are collapsed: e.g. `<<<Special comment section>>>`
    /// matches `"special comment\n  section"`.
    fn try_parse_radio_link(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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
            if let Some(match_end) = match_radio_target_text(text, target) {
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
                    application: None,
                    flags: LinkFlags::new(LinkFormat::Plain, LinkType::Radio),
                    path: target,
                    raw_link: raw,
                    search_option: None,
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

    fn try_parse_target(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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
        let valid_pre =
            start == self.object_region_start || is_pre_char(self.input.as_bytes()[start - 1]);
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
            if c == NBSP || c == ZWS || c.is_ascii_whitespace() {
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
                                c == NBSP || c == ZWS || c.is_ascii_whitespace()
                            });
                            if !is_ws {
                                let valid_post = i + 1 >= text.len() || is_post_char(bytes[i + 1]);
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

        let content_location = Interval {
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

    fn try_parse_plain_link(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        let proto_len = plain_link_proto_len(bytes, self.environment.link_types())?;
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
                    '.' | ',' | ':' | ';' | '!' | '?' | '\u{00AB}' | '\u{00BB}'
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

    /// Parse an angle link: `<protocol://...>`.
    fn try_parse_angle_link(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    /// Parse a citation object: `[cite/style:@key1; @key2]`.
    fn try_parse_citation(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    /// Parse a statistics cookie: `[n/m]`, `[n/]`, `[/m]`, `[/]` or `[n%]`,
    /// `[%]`. Mirrors org-element's `\[[0-9]*\(?:%\|/[0-9]*\)\]`.
    fn try_parse_macro(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    fn try_parse_inline_babel_call(
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
        let inside_header: Option<&'a str> = None;

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
                    inside_header,
                    arguments,
                    end_header: None,
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

    fn try_parse_statistics_cookie(
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

    fn try_parse_footnote_reference(
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

    fn try_parse_plain_text(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let mut consume = scan_plain_text_end(
            text.as_bytes(),
            self.environment.link_types(),
            self.environment.link_start_bytes(),
        );
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
                    pos += 1;
                    continue;
                }
            }

            // Check each target starting at this position
            for target in &self.radio_targets {
                if let Some(match_end) = match_radio_target_text(&text[pos..], target) {
                    // Word boundary after: byte at `match_end` must be
                    // non-alphanumeric or end of buffer.
                    let next_pos = pos + match_end;
                    if next_pos < limit {
                        let next = bytes[next_pos];
                        if next.is_ascii_alphanumeric() || next == b'_' {
                            continue;
                        }
                    }
                    // Word boundary after
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
        }

        None
    }

    /// Parse a timestamp from `text` and return the data + bytes consumed.
    /// Does NOT allocate an arena node — use [`try_parse_timestamp`] when a
    /// node is needed in the parse tree.
    #[inline(never)]
    pub fn parse_timestamp(&self, text: &'a str) -> Option<(TimestampData<'a>, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || (bytes[0] != b'<' && bytes[0] != b'[') {
            return None;
        }

        // Links [[...]] and targets <<...>> start with the same delimiter
        // character as timestamps, so reject the double-delimiter form here.
        if bytes.len() > 1 && bytes[0] == bytes[1] {
            return None;
        }

        let closing = if bytes[0] == b'<' { b'>' } else { b']' };

        // Find closing bracket
        let mut found_close = None;
        let mut search_pos = 1;
        while search_pos < text.len() {
            match memchr(closing, &bytes[search_pos..]) {
                Some(offset) => {
                    let i = search_pos + offset;
                    if i + 1 >= text.len() || is_post_char(bytes[i + 1]) {
                        found_close = Some(i);
                        break;
                    }
                    search_pos = i + 1;
                }
                None => break,
            }
        }

        let close = found_close?;

        let raw = &text[..close + 1];
        let timestamp_data = TimestampData::new(raw)?;
        Some((timestamp_data, close + 1))
    }

    #[expect(
        clippy::missing_inline_in_public_items,
        reason = "This function is both huge, so adding `inline` to it might cause problems downstream, and on the hot path, meaning that `inline(never)` would disable inlining"
    )]
    pub fn try_parse_timestamp(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let (timestamp_data, consumed) = self.parse_timestamp(text)?;

        // Absorb trailing spaces/tabs into the timestamp's post-blank,
        // matching Emacs org-element behaviour.
        let post_blank = text[consumed..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let total = consumed + post_blank;

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Timestamp(self.bump.alloc(timestamp_data)),
                (start, start + total),
                self.bump,
            )
            .content((start + 1, start + consumed - 1))
            .post_blank(post_blank)
            .build(),
        );

        Some((node, total))
    }

    fn try_parse_entity(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    /// Parse a LaTeX fragment from `text`, consuming the fragment text and
    /// any trailing whitespace (post-blank).
    ///
    /// Recognised patterns:
    ///
    /// 1. `\command{...}`           — backslash + letters (optionally `*`) + brace group
    /// 2. `\command[...]{...}`      — with optional bracket argument
    /// 3. `\command`                — bare command (no arguments)
    ///
    /// Parse an Org line break: exactly `\\` at the end of a line (only
    /// `[ \t]` may follow before the newline), where the `\\` is not part of a
    /// longer backslash run. The node spans through the trailing whitespace
    /// and newline.
    fn try_parse_line_break(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    fn try_parse_latex_fragment(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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

    fn try_parse_latex_math(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
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
