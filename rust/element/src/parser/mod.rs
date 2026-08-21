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

mod dispatch;
mod objects;
#[cfg(feature = "par-parse")]
pub(crate) mod par;

use memchr::{memchr, memmem, memrchr};

use crate::{
    cursor::Cursor,
    data::{BumpVec, ChunkArena, Interval, NodeId, Syntax, SyntaxNode, SyntaxT, TimestampData},
    environment,
    list::ListStruct,
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
    /// Content of the first section (before the first headline).  Emacs calls
    /// this `top-comment`: it permits a leading property drawer or a property
    /// drawer immediately following a top comment, but does not itself open a
    /// new section.
    TopComment,
    Section,
    Planning,
    Item,
    NodeProperty,
    TableRow,
    PropertyDrawer,
    FootnoteDefinition,
}

pub struct Parser<'input, 'bumpalo, Environment: environment::Environment> {
    pub cursor: Cursor<'input>,
    pub input: &'input str,
    pub granularity: ParseGranularity,
    pub environment: Environment,
    pub arena: ChunkArena<'input, 'bumpalo>,
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
    let bytes = input.as_bytes();

    // Radio targets are rare: jump directly to each `<<<` occurrence with a
    // single shared searcher rather than walking every line of the buffer
    // (which, on a large file with even one `<<<`, costs more than the entire
    // parse).  For each occurrence we resolve the containing line and reuse the
    // existing per-line extraction logic, deduplicating lines so a line with
    // multiple `<<<` is processed at most once.
    let mut targets: Vec<&str> = Vec::new();
    let mut processed_line_end = 0usize;

    for pos in memmem::find_iter(bytes, b"<<<") {
        // Skip occurrences that fall inside a line we've already handled.
        if pos < processed_line_end {
            continue;
        }
        let line_start = match memrchr(b'\n', &bytes[..pos]) {
            Some(i) => i + 1,
            None => 0,
        };
        let line_end = match memchr(b'\n', &bytes[pos..]) {
            Some(i) => pos + i,
            None => bytes.len(),
        };
        // Advance past this line so later occurrences on it are skipped.
        processed_line_end = line_end + 1;

        // Match `str::lines()` semantics, which strips a trailing `\r`.
        let mut line = &input[line_start..line_end];
        if let Some(stripped) = line.strip_suffix('\r') {
            line = stripped;
        }

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
            arena: ChunkArena::new(),
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
                // Recursing into the *first* section enters top-comment mode
                // (Emacs: `first-section` → `top-comment`), which permits the
                // leading property-drawer / top-comment shortcut without opening
                // another section.  Every other section advances to planning.
                Section if mode == ParserMode::FirstSection => ParserMode::TopComment,
                Section => ParserMode::Planning,
                Table => ParserMode::TableRow,
                FootnoteDefinition => ParserMode::FootnoteDefinition,
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
                (ParserMode::TopComment, Comment) => PmPropertyDrawer,
                // Any other element ends the top-comment shortcut (Emacs: nil).
                (ParserMode::TopComment, _) => PmNil,
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
    pub fn parse_buffer(&mut self) -> (ChunkArena<'a, 'b>, NodeId) {
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

        let arena = std::mem::take(&mut self.arena);
        (arena, root)
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
                    .iter()
                    .all(|&b| b == b' ' || b == b'\t' || b == b'\n')
                {
                    self.cursor.set(line_end);
                    continue;
                }
            }

            let element = self.current_element(span.end, mode, structure);

            let node_ref = self.arena.get(element);
            let element_end = node_ref.location.end;
            let element_content = node_ref.content_location;
            let is_greater = SyntaxT::from(&node_ref.data).is_greater_element();
            let data_disc = SyntaxT::from(&node_ref.data);
            let list_struct = match &node_ref.data {
                Syntax::PlainList(d) => Some(d.structure),
                _ => None,
            };
            self.cursor.set(element_end);

            if let Some(content_location) = element_content {
                if is_greater {
                    let recurse = (SyntaxT::Headline == data_disc)
                        || (self.granularity == ParseGranularity::Element
                            || self.granularity == ParseGranularity::Object)
                        || ((SyntaxT::Section == data_disc)
                            && (self.granularity == ParseGranularity::GreaterElement));

                    if recurse {
                        let new_mode = Parser::<Environment>::next_mode(mode, data_disc, true);

                        let mut children =
                            self.parse_elements(content_location, new_mode, list_struct);
                        // Emacs creates a paragraph for a blank-only region inside
                        // quote / center / special blocks (the lone blank line between
                        // `#+BEGIN_QUOTE` and `#+END_QUOTE` is a one-newline-wide
                        // paragraph).  `parse_elements` skips blank lines at section
                        // level, so when the content is blank and we are inside one
                        // of these blocks, synthesise the expected paragraph.
                        if children.is_empty()
                            && matches!(
                                data_disc,
                                SyntaxT::QuoteBlock | SyntaxT::CenterBlock | SyntaxT::SpecialBlock
                            )
                        {
                            let content = content_location;
                            let content_bytes = &self.input.as_bytes()[content.start..content.end];
                            if !content_bytes.is_empty()
                                && content_bytes
                                    .iter()
                                    .all(|&b| b == b' ' || b == b'\t' || b == b'\n')
                            {
                                let para_objects = self.parse_objects(content, |_| true);
                                let para = self.arena.alloc_with_children(
                                    SyntaxNode::new(Syntax::Paragraph, content, self.bump)
                                        .content(content)
                                        .build(),
                                    para_objects,
                                );
                                children.push(para);
                            }
                        }
                        self.arena.set_children(element, children);
                    }
                } else if let ParseGranularity::Object = &self.granularity {
                    let children =
                        self.parse_objects(content_location, |that| data_disc.can_contain(that));
                    self.arena.set_children(element, children);
                }
            }

            if self.granularity == ParseGranularity::Object {
                let node = self.arena.get(element);
                let title_location = if let Syntax::Headline(ref data) = node.data {
                    data.title_location
                } else {
                    None
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

    /// Parse a timestamp from `text` and return the data + bytes consumed.
    /// Does not allocate an arena node; the object parser wraps this when a
    /// node is needed in the parse tree. Lives in the core because planning
    /// and clock lines contain timestamps at Element granularity.
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
}
