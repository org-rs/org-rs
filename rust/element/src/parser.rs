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

use std::rc::Rc;

use memchr::{memchr, memchr2, memchr3, memmem};

use crate::affiliated::ElementSpan;
use crate::babel::REGEX_BABEL_CALL;
use crate::cursor::Cursor;
use crate::data::{
    Brackets, EntityData, FootnoteReferenceData, Interval, LinkData, NodeArena, NodeId, ScriptFlags,
    ScriptKind, Syntax, SyntaxNode, SyntaxT, TimestampData,
};

use crate::blocks::{REGEX_BLOCK_BEGIN, REGEX_DYNAMIC_BLOCK};
use crate::drawer::REGEX_DRAWER;
use crate::headline::{
    REGEX_CLOCK_LINE, REGEX_PLANNING_LINE, REGEX_PROPERTY_DRAWER,
};
use crate::keyword::*;
use crate::latex::REGEX_LATEX_BEGIN_ENVIRIONMENT;
use crate::list::*;
use crate::markup::REGEX_DIARY_SEXP;
use crate::markup::REGEX_FOOTNOTE_DEFINITION;
use crate::markup::REGEX_HORIZONTAL_RULE;
use crate::table::REGEX_TABLE_BORDER;

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
    FirstSection,
    Section,
    Planning,
    Item,
    NodeProperty,
    TableRow,
    PropertyDrawer,
}

pub struct Parser<'a, Environment: crate::environment::Environment> {
    pub cursor: Cursor<'a>,
    pub input: &'a str,
    pub granularity: ParseGranularity,
    pub environment: Environment,
    pub arena: NodeArena<'a>,
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
        b' ' | b'\t' | b'\n' | b'.' | b',' | b'!' | b'?' | b';' | b':'
            | b'\'' | b')' | b'}' | b'\\' | b'[' | b'-'
    )
}

/// Scan `bytes` for the first position that ends a plain-text run, returning
/// the number of bytes that belong to the run.
///
/// Four independent SIMD cursors track the next candidate byte for each
/// mutually-exclusive character group.  When a candidate fails its contextual
/// check only that group's cursor is advanced; the other three retain their
/// previously found positions, eliminating the rescanning that a single shared
/// offset would cause.
///
/// Groups and their stop conditions:
/// - `[` — always a stop (link / footnote / timestamp opener)
/// - `*` `/` `_` — stop when preceded by a pre-char
/// - `+` `=` `~` — stop when preceded by a pre-char
/// - `h` `f` `m` — stop when preceded by a pre-char and followed by a
///   recognised URL protocol prefix
///
/// Returns zero when no plain-text bytes are available at the start of
/// `bytes`.
#[inline]
fn scan_plain_text_end(bytes: &[u8]) -> usize {
    let p1 = memchr(b'[', bytes);
    let mut p2 = memchr2(b'*', b'/', bytes);
    let mut p3 = memchr3(b'+', b'=', b'~', bytes);
    let mut p4 = memchr3(b'h', b'f', b'm', bytes);
    let mut p5 = memchr2(b'_', b'^', bytes);

    loop {
        let i = match [p1, p2, p3, p4, p5].iter().copied().flatten().min() {
            None => return bytes.len(),
            Some(pos) => pos,
        };
        let b = bytes[i];

        if b == b'[' {
            return i;
        }
        if matches!(b, b'*' | b'/' | b'+' | b'=' | b'~')
            && i > 0
            && is_pre_char(bytes[i - 1])
        {
            return i;
        }
        if matches!(b, b'_' | b'^') && i > 0 {
            return i;
        }
        if matches!(b, b'h' | b'f' | b'm')
            && (i == 0 || is_pre_char(bytes[i - 1]))
            && (bytes[i..].starts_with(b"https://")
                || bytes[i..].starts_with(b"http://")
                || bytes[i..].starts_with(b"ftp://")
                || bytes[i..].starts_with(b"mailto:"))
        {
            return i;
        }

        let next = i + 1;
        let rest = &bytes[next..];
        match b {
            b'[' => unreachable!(),
            b'*' | b'/' => p2 = memchr2(b'*', b'/', rest).map(|r| next + r),
            b'+' | b'=' | b'~' => p3 = memchr3(b'+', b'=', b'~', rest).map(|r| next + r),
            b'_' | b'^' => p5 = memchr2(b'_', b'^', rest).map(|r| next + r),
            _ => p4 = memchr3(b'h', b'f', b'm', rest).map(|r| next + r),
        }
    }
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    #[inline]
    pub fn new(
        input: &'a str,
        granularity: ParseGranularity,
        environment: Environment,
    ) -> Parser<'a, Environment> {
        Parser {
            cursor: Cursor::new(input, 0),
            input,
            granularity,
            environment,
            arena: NodeArena::new(),
        }
    }

    /// Returns parser mode according to given `element` and `is_parent`
    /// `element` is AllElements variant representing the type of an element
    /// containing next element if `is_parent` is true, or before it
    /// otherwise.
    /// <br>
    /// Original function name: org-element--next-mode
    /// https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L4273
    fn next_mode(syntax: SyntaxT, is_parent: bool) -> Option<ParserMode> {
        use SyntaxT::*;

        if is_parent {
            match syntax {
                Headline => Some(ParserMode::Section),
                InlineTask => Some(ParserMode::Planning),
                Item => Some(ParserMode::Planning),
                PlainList => Some(ParserMode::Item),
                PropertyDrawer => Some(ParserMode::NodeProperty),
                Section => Some(ParserMode::Planning),
                Table => Some(ParserMode::TableRow),
                _ => None,
            }
        } else {
            match syntax {
                Item => Some(ParserMode::Item),
                NodeProperty => Some(ParserMode::NodeProperty),
                Planning => Some(ParserMode::PropertyDrawer),
                TableRow => Some(ParserMode::TableRow),
                _ => None,
            }
        }
    }

    /// org-element-parse-buffer
    /// Parses input from beginning to the end
    #[inline]
    pub fn parse_buffer(&mut self) -> (NodeArena<'a>, NodeId) {
        self.cursor.set(0);
        self.cursor.skip_whitespace();

        let end = self.input.len();
        let root = self.arena.alloc(SyntaxNode::create_root());
        let children = self.parse_elements((0, end), ParserMode::FirstSection, None);
        self.arena.set_children(root, children);

        (std::mem::take(&mut self.arena), root)
    }

    /// Parse elements between BEG and END positions.
    #[inline]
    pub fn parse_elements(
        &mut self,
        span: impl Into<Interval>,
        mut mode: ParserMode,
        structure: Option<Rc<ListStruct<'a>>>,
    ) -> Vec<NodeId> {
        let span = span.into();
        let pos = self.cursor.pos();
        self.cursor.set(span.start);

        if self.granularity == ParseGranularity::Headline && !self.cursor.on_headline() {
            self.cursor.next_headline();
        }

        let mut elements: Vec<NodeId> = vec![];
        loop {
            let current_pos = self.cursor.pos();
            if current_pos >= span.end {
                break;
            }

            {
                let line_end = memchr(b'\n', &self.input.as_bytes()[current_pos..span.end])
                    .map_or(span.end, |i| current_pos + i + 1);
                if self.input[current_pos..line_end].trim().is_empty() {
                    self.cursor.set(line_end);
                    continue;
                }
            }

            let list_struct = structure.as_ref().map(|rc| rc.clone());
            let element = self.current_element(span.end, mode, list_struct);

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
                                Syntax::PlainList(d) => Some(d.structure.clone()),
                                _ => None,
                            }
                        };

                        let new_mode =
                            Parser::<Environment>::next_mode(data_disc, true)
                                .unwrap_or(mode);

                        let children = self.parse_elements(content_location, new_mode, list_sturct);
                        self.arena.set_children(element, children);
                    }
                } else if let ParseGranularity::Object = &self.granularity {
                    let children = self.parse_objects(
                        content_location,
                        |that| data_disc.can_contain(that),
                    );
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
                    let title_objects = self.parse_objects(
                        loc,
                        |that| SyntaxT::Headline.can_contain(that),
                    );
                    self.arena.set_title_objects(element, title_objects);
                }
            }

            {
                let node = self.arena.get(element);
                if let Some(m) = Parser::<Environment>::next_mode(SyntaxT::from(&node.data), false) {
                    mode = m
                }
            }
            elements.push(element);
        }
        self.cursor.set(pos);
        elements
    }

    /// Parse the element starting at cursor position (point).
    #[inline]
    pub fn current_element(
        &mut self,
        limit: usize,
        mode: ParserMode,
        structure: Option<Rc<ListStruct<'a>>>,
    ) -> NodeId {
        let pos = self.cursor.pos();

        let raw_secondary_p = self.granularity == ParseGranularity::Object;

        let get_current_element = || -> NodeId {
            use crate::parser::ParserMode::*;

            if mode == Item {
                return self.item_parser(structure, raw_secondary_p);
            }

            if mode == TableRow {
                return self.table_row_parser();
            }

            if mode == NodeProperty {
                return self.node_property_parser(limit);
            }

            if self.cursor.on_headline() {
                return self.headline_parser();
            }

            if mode == Section || mode == FirstSection {
                let p = self.cursor.pos();
                let lim = self.cursor.next_headline().unwrap_or(limit).min(limit);
                self.cursor.set(p);
                return self.section_parser(lim);
            }

            {
                let maybe_headline_offset = self.cursor.line_beginning_position(Some(0));
                let is_prev_line_headline =
                    self.input.as_bytes().get(maybe_headline_offset) == Some(&b'*');
                let is_match_planning = self.cursor.looking_at(&*REGEX_PLANNING_LINE).is_some();

                if mode == Planning && is_prev_line_headline && is_match_planning {
                    return self.planning_parser(limit);
                }
            }

            {
                let delta = if mode == Planning { 0 } else { -1 };
                let maybe_headline_offset = self.cursor.line_beginning_position(Some(delta));
                let is_prev_line_headline =
                    self.input.as_bytes().get(maybe_headline_offset) == Some(&b'*');

                if (mode == Planning || mode == PropertyDrawer)
                    && is_prev_line_headline
                    && self.cursor.looking_at(&*REGEX_PROPERTY_DRAWER).is_some()
                {
                    return self.property_drawer_parser(limit);
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
            let span = if b == Some(b'#') {
                self.collect_affiliated_keywords(limit)
            } else {
                ElementSpan::new((cur, limit)).build()
            };

            // If affiliated keywords were consumed and we're past the limit,
            // they're orphaned → standalone keyword.
            if span.affiliated.is_some() && self.cursor.pos() >= limit {
                self.cursor.set(span.span.start);
                return self.keyword_parser(
                    ElementSpan::new((span.span.start, limit)).build(),
                );
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
                    rest.iter().position(|&c| c != b' ' && c != b'\t')
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
                            b"center"  => self.center_block_parser(span),
                            b"comment" => self.comment_block_parser(span),
                            b"example" => self.example_block_parser(span),
                            b"export"  => self.export_block_parser(span),
                            b"quote"   => self.quote_block_parser(span),
                            b"src"     => self.src_block_parser(span),
                            b"verse"   => self.verse_block_parser(span),
                            _          => self.special_block_parser(span),
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
                        return self.drawer_parser(span);
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
                #[allow(clippy::collapsible_match)]
                Some(b'-') => {
                    if looking_at!(REGEX_HORIZONTAL_RULE, self).is_some() {
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

            // Item (handles leading whitespace via its own regex)
            if looking_at!(REGEX_ITEM, self).is_some() {
                let s = structure.unwrap_or(self.list_struct(limit));
                return self.plain_list_parser(span, s.clone());
            }

            self.paragraph_parser(span)
        };

        let current_element = get_current_element();
        self.cursor.set(pos);
        current_element
    }

    /// Parse objects between `beg` and `end` and return recursive structure.
    #[inline]
    pub fn parse_objects(
        &mut self,
        interval: impl Into<Interval>,
        restriction: impl Fn(SyntaxT) -> bool,
    ) -> Vec<NodeId> {
        let interval = interval.into();
        let mut children: Vec<NodeId> = Vec::new();
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
                            if restriction(SyntaxT::Bold) { children.push(n); }
                            c
                        }),
                        b'/' => self.try_parse_italic(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Italic) { children.push(n); }
                            c
                        }),
                        b'~' => self.try_parse_code(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Code) { children.push(n); }
                            c
                        }),
                        b'=' => self.try_parse_verbatim(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Verbatim) { children.push(n); }
                            c
                        }),
                        b'_' => {
                            if let Some((n, c)) = self.try_parse_script(remaining, pos, ScriptKind::Sub) {
                                if restriction(SyntaxT::Script) { children.push(n); }
                                Some(c)
                            } else {
                                self.try_parse_underline(remaining, pos).map(|(n, c)| {
                                    if restriction(SyntaxT::Underline) { children.push(n); }
                                    c
                                })
                            }
                        }
                        b'^' => self.try_parse_script(remaining, pos, ScriptKind::Sup).map(|(n, c)| {
                            if restriction(SyntaxT::Script) { children.push(n); }
                            c
                        }),
                        b'+' => self.try_parse_strikethrough(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::StrikeThrough) { children.push(n); }
                            c
                        }),
                        b'[' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_footnote_reference(remaining, pos) {
                                if restriction(SyntaxT::FootnoteReference) { children.push(n); }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_timestamp(remaining, pos) {
                                if restriction(SyntaxT::Timestamp) { children.push(n); }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_link(remaining, pos) {
                                if restriction(SyntaxT::Link) { children.push(n); }
                                result = Some(c);
                            }
                            result
                        }
                        b'\\' => self.try_parse_entity(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Entity) { children.push(n); }
                            c
                        }),
                        b'<' => {
                            let mut result = None;
                            if let Some((n, c)) = self.try_parse_timestamp(remaining, pos) {
                                if restriction(SyntaxT::Timestamp) { children.push(n); }
                                result = Some(c);
                            } else if let Some((n, c)) = self.try_parse_target(remaining, pos) {
                                if restriction(SyntaxT::Target) { children.push(n); }
                                result = Some(c);
                            }
                            result
                        }
                        b'h' | b'f' | b'm' => self.try_parse_plain_link(remaining, pos).map(|(n, c)| {
                            if restriction(SyntaxT::Link) { children.push(n); }
                            c
                        }),
                        _ => None,
                    }
                }
            };

            if let Some(c) = consumed {
                pos += c;
            } else if let Some((node, c)) = self.try_parse_plain_text(remaining, pos) {
                children.push(node);
                pos += c;
            } else {
                pos += 1;
            }
        }

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

    fn try_parse_script(&mut self, text: &'a str, start: usize, kind: ScriptKind) -> Option<(NodeId, usize)> {
        let _ = start.checked_sub(1)
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
            &b if b.is_ascii_alphanumeric() => {
                let len = bytes[1..].iter()
                    .take_while(|&&b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'+' | b'.' | b','))
                    .count();
                (Brackets::Bare, 1 + len)
            }
            _ => return None,
        };

        let flags = ScriptFlags::new(kind, brackets);
        let node = self.arena.alloc_with_children(
            SyntaxNode::new(Syntax::Script(flags), (start, start + consumed)).build(),
            vec![],
        );
        Some((node, consumed))
    }

    fn try_parse_link(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 4 || &bytes[0..2] != b"[[" {
            return None;
        }

        // Find closing ]]
        let mut found_close = None;
        let mut search_pos = 2;
        while search_pos + 1 < bytes.len() {
            match memchr(b']', &bytes[search_pos..]) {
                Some(offset) => {
                    let i = search_pos + offset;
                    if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                        let after = i + 2;
                        let valid_post =
                            after >= bytes.len() || is_post_char(bytes[after]);
                        if valid_post {
                            found_close = Some(after);
                            break;
                        }
                        search_pos = i + 2;
                    } else {
                        search_pos = i + 1;
                    }
                }
                None => break,
            }
        }

        let close = found_close?;
        let raw = &text[..close];

        // Detect the optional description: the ][ separator between target and
        // description within [[target][description]].  The description is in
        // org-element-contents (unlike the headline title), so it is parsed
        // into child objects.
        let inner = &text[2..close - 2];
        let desc_loc = memmem::find(inner.as_bytes(), b"][").map(|sep| Interval {
            start: start + 2 + sep + 2,
            end:   start + close - 2,
        });

        let link_data = LinkData::new(raw);

        let node = self.arena.alloc(
            SyntaxNode::new(Syntax::Link(Box::new(link_data)), (start, start + close))
                .content((start + 2, start + close - 2))
                .build(),
        );

        if let Some(desc) = desc_loc {
            if desc.start < desc.end {
                let children = self.parse_objects(desc, |that| SyntaxT::Link.can_contain(that));
                self.arena.set_children(node, children);
            }
        }

        Some((node, close))
    }

    fn try_parse_target(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 4 || &bytes[0..2] != b"<<" {
            return None;
        }

        // Find closing >>
        let mut found_close = None;
        let mut search_pos = 2;
        while search_pos + 1 < bytes.len() {
            match memchr(b'>', &bytes[search_pos..]) {
                Some(offset) => {
                    let i = search_pos + offset;
                    if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                        let after = i + 2;
                        let valid_post =
                            after >= bytes.len() || is_post_char(bytes[after]);
                        if valid_post {
                            found_close = Some(after);
                            break;
                        }
                        search_pos = i + 2;
                    } else {
                        search_pos = i + 1;
                    }
                }
                None => break,
            }
        }

        let close = found_close?;
        let content = &text[2..close - 2];

        let node = self.arena.alloc(
            SyntaxNode::new(Syntax::Target(content), (start, start + close)).build(),
        );

        Some((node, close))
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

        // Check PRE condition: start of text (position 0 in slice) is always valid
        // For non-start positions, would need to check previous char
        let valid_pre = true; // At start of content, always valid
        if !valid_pre {
            return None;
        }

        // Need at least: marker + 1 content + closing marker
        if text.len() < 3 {
            return None;
        }

        // First content char must be non-whitespace
        if bytes[1] == b' ' || bytes[1] == b'\t' || bytes[1] == b'\n' {
            return None;
        }

        let mut newlines = 0u8;
        let mut found_close = None;
        let mut pos = 2;

        // Search for closing marker
        while pos < text.len() {
            match memchr2(b'\n', marker, &bytes[pos..]) {
                Some(offset) => {
                    let i = pos + offset;
                    if bytes[i] == b'\n' {
                        newlines += 1;
                        if newlines > 1 {
                            break;
                        }
                        pos = i + 1;
                    } else {
                        // Found marker - last content char must be non-whitespace
                        let prev = bytes[i - 1];
                        if prev != b' ' && prev != b'\t' && prev != b'\n' {
                            let valid_post =
                                i + 1 >= text.len() || is_post_char(bytes[i + 1]);
                            if valid_post {
                                found_close = Some(i);
                                break;
                            }
                        }
                        pos = i + 1;
                    }
                }
                None => break,
            }
        }

        let close = found_close?;

        let content_location = Interval { start: start + 1, end: start + close };
        let content = &self.input[content_location.start..content_location.end];

        let (data, children) = match syntax {
            SyntaxT::Bold          => (Syntax::Bold,              self.parse_objects(content_location, |_| true)),
            SyntaxT::Italic        => (Syntax::Italic,            self.parse_objects(content_location, |_| true)),
            SyntaxT::Underline     => (Syntax::Underline,         self.parse_objects(content_location, |_| true)),
            SyntaxT::StrikeThrough => (Syntax::StrikeThrough,     self.parse_objects(content_location, |_| true)),
            SyntaxT::Code          => (Syntax::Code(content),     vec![]),
            SyntaxT::Verbatim      => (Syntax::Verbatim(content), vec![]),
            _                      => return None,
        };

        let post_blank = bytes.get(close + 1..)
            .map(|rest| rest.iter().take_while(|&&b| b == b' ' || b == b'\t').count())
            .unwrap_or(0);

        let node = self.arena.alloc_with_children(
            SyntaxNode::new(data, (start, start + close + 1 + post_blank))
                .content(content_location)
                .build(),
            children,
        );

        Some((node, close + 1 + post_blank))
    }

    fn try_parse_plain_link(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        const PROTOCOLS: &[&[u8]] = &[b"https://", b"http://", b"ftp://", b"mailto:"];
        let bytes = text.as_bytes();
        let proto_len = PROTOCOLS.iter().find_map(|&p| {
            if bytes.starts_with(p) {
                Some(p.len())
            } else {
                None
            }
        })?;
        let url_end = text[proto_len..]
            .find(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | '<' | '>'))
            .map_or(text.len(), |i| proto_len + i);
        if url_end <= proto_len {
            return None;
        }
        let raw = &text[..url_end];
        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Link(Box::new(LinkData::new_plain(raw))),
                (start, start + url_end),
            )
            .build(),
        );
        Some((node, url_end))
    }

    fn try_parse_footnote_reference(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        if !text.starts_with("[fn:") {
            return None;
        }
        let close = text.find(']')?;
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
            .unwrap_or_default();

        let consumed = close + 1;
        let mut builder = SyntaxNode::new(
            Syntax::FootnoteReference(Box::new(FootnoteReferenceData { label, type_s })),
            (start, start + consumed),
        );
        if let Some(loc) = definition_location {
            builder = builder.content(loc);
        }
        let node = self.arena.alloc_with_children(builder.build(), children);
        Some((node, consumed))
    }

    fn try_parse_plain_text(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let consume = scan_plain_text_end(text.as_bytes());
        if consume == 0 {
            return None;
        }
        let node = self.arena.alloc(
            SyntaxNode::new(Syntax::PlainText(&text[..consume]), (start, start + consume)).build(),
        );
        Some((node, consume))
    }

    /// Parse a timestamp from `text` and return the data + bytes consumed.
    /// Does NOT allocate an arena node — use [`try_parse_timestamp`] when a
    /// node is needed in the parse tree.
    #[inline]
    pub fn parse_timestamp(&self, text: &'a str) -> Option<(TimestampData<'a>, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || (bytes[0] != b'<' && bytes[0] != b'[') {
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

    #[inline]
    pub fn try_parse_timestamp(&mut self, text: &'a str, start: usize) -> Option<(NodeId, usize)> {
        let (timestamp_data, consumed) = self.parse_timestamp(text)?;

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Timestamp(Box::new(timestamp_data)),
                (start, start + consumed),
            )
            .content((start + 1, start + consumed - 1))
            .build(),
        );

        Some((node, consumed))
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
        let entity_data = EntityData::new(entity_name)?;

        let node = self.arena.alloc(
            SyntaxNode::new(
                Syntax::Entity(Box::new(entity_data)),
                (start, start + end),
            )
            .build(),
        );

        Some((node, end))
    }
}
