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

use std::cell::RefCell;
use std::rc::Rc;

use regex::Regex;

use crate::babel::REGEX_BABEL_CALL;
use crate::cursor::Cursor;
use crate::data::{
    CodeData, EntityData, FootnoteReferenceData, Interval, LinkData, Syntax, SyntaxNode, SyntaxT,
    TargetData, TimestampData, VerbatimData,
};
use crate::environment::Environment;

use crate::blocks::{
    REGEX_BLOCK_BEGIN, REGEX_COLON_OR_EOL, REGEX_DYNAMIC_BLOCK, REGEX_STARTS_WITH_HASHTAG,
};
use crate::drawer::REGEX_DRAWER;
use crate::headline::{
    REGEX_CLOCK_LINE, REGEX_HEADLINE_SHORT, REGEX_PLANNING_LINE, REGEX_PROPERTY_DRAWER,
};
use crate::keyword::*;
use crate::latex::REGEX_LATEX_BEGIN_ENVIRIONMENT;
use crate::list::*;
use crate::markup::REGEX_DIARY_SEXP;
use crate::markup::REGEX_FIXED_WIDTH;
use crate::markup::REGEX_FOOTNOTE_DEFINITION;
use crate::markup::REGEX_HORIZONTAL_RULE;
use crate::table::{REGEX_TABLE_BORDER, REGEX_TABLE_PRE_BORDER, REGEX_TABLE_RULE};

/// determines the depth of the recursion.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
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
    pub cursor: RefCell<Cursor<'a>>,
    pub input: &'a str,
    pub granularity: ParseGranularity,
    pub environment: Environment,
}

macro_rules! looking_at {
    ($regex:ident, $parser: ident) => {
        $parser.cursor.borrow_mut().looking_at(&*$regex)
    };
}

macro_rules! capturing_at {
    ($regex:ident, $parser: ident) => {
        $parser.cursor.borrow_mut().capturing_at(&*$regex)
    };
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    pub fn new(
        input: &'a str,
        granularity: ParseGranularity,
        environment: Environment,
    ) -> Parser<Environment> {
        Parser {
            cursor: RefCell::new(Cursor::new(input, 0)),
            input,
            granularity,
            environment,
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
    pub fn parse_buffer(&'a self) -> SyntaxNode {
        self.cursor.borrow_mut().set(0);
        self.cursor.borrow_mut().skip_whitespace();

        let end = self.input.len();
        let mut root = SyntaxNode::create_root();
        root.children = RefCell::new(self.parse_elements(0, end, ParserMode::FirstSection, None));
        root
    }

    /// Parse elements between BEG and END positions.
    /// https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L4340
    ///
    /// MODE prioritizes some elements over the others.  It can be set to
    /// `first-section', `section', `planning', `item', `node-property'
    /// or `table-row'.
    ///
    /// When value is `item', STRUCTURE will be used as the current list
    /// structure.
    ///
    /// Elements are accumulated into ACC."
    /// (defun org-element--parse-elements
    ///     (beg end mode structure granularity visible-only acc)
    /// TODO do not forget to fix child-parent and parent-child links on tree updates
    fn parse_elements(
        &'a self,
        beg: usize,
        end: usize,
        mut mode: ParserMode,
        structure: Option<Rc<ListStruct>>,
    ) -> Vec<Rc<SyntaxNode>> {
        let pos = self.cursor.borrow_mut().pos();
        self.cursor.borrow_mut().set(beg);

        // When parsing only headlines, skip any text before first one.
        if self.granularity == ParseGranularity::Headline && !self.cursor.borrow_mut().on_headline()
        {
            self.cursor.borrow_mut().next_headline();
        }

        let mut elements: Vec<Rc<SyntaxNode>> = vec![];
        loop {
            let current_pos = self.cursor.borrow().pos();
            if current_pos >= end {
                break;
            }

            // Find current element's type and parse it accordingly to its category.
            // (org-element--current-element end granularity mode structure))
            let list_struct = match &structure {
                None => None,
                Some(rc) => Some(rc.clone()),
            };
            let element: SyntaxNode = self.current_element(end, mode, list_struct);

            // (goto-char (org-element-property :end element))
            self.cursor.borrow_mut().set(element.location.end);

            // Recurse into element's children if it has contents
            if element.content_location.is_some() {
                let content_location = element.content_location.unwrap();

                // If this is a Greater element:
                // parse it between `contents_begin' and `contents_end'
                // if one the following conditions holds:
                // 1. This is a headline - going inside is mandatory,
                //    in order to get sub-level headings.
                // 2. Granularity is Element or Object
                // 3. This is Section and Granularity is GreaterElement
                if SyntaxT::from(&element.data).is_greater_element() {
                    if (SyntaxT::Headline == SyntaxT::from(&element.data))
                        || (self.granularity == ParseGranularity::Element
                            || self.granularity == ParseGranularity::Object)
                        || ((SyntaxT::Section == SyntaxT::from(&element.data))
                            && (self.granularity == ParseGranularity::GreaterElement))
                    {
                        // (and (memq type '(item plain-list))
                        // (org-element-property :structure element))
                        let list_sturct = match &element.data {
                            Syntax::PlainList(d) => Some(d.structure.clone()),
                            _ => None,
                        };

                        //  Possibly switch to a special mode.
                        // (org-element--next-mode type t)
                        let new_mode =
                            Parser::<Environment>::next_mode(SyntaxT::from(&element.data), true)
                                .unwrap_or(mode);

                        element.children.replace(self.parse_elements(
                            content_location.start,
                            content_location.end,
                            new_mode,
                            list_sturct,
                        ));
                    }
                }
                // Any other element with contents, if granularity allows it
                else {
                    // (org-element--parse-objects
                    //    cbeg (org-element-property :contents-end element)
                    //    element (org-element-restriction type))))
                    if let ParseGranularity::Object = &self.granularity {
                        element.children.replace(self.parse_objects(
                            content_location.start,
                            content_location.end,
                            |that| SyntaxT::from(&element.data).can_contain(that),
                        ));
                    }
                }
            }
            if let Some(m) = Parser::<Environment>::next_mode(SyntaxT::from(&element.data), false) {
                mode = m
            }
            elements.push(Rc::new(element));
        }
        self.cursor.borrow_mut().set(pos);
        elements
    }

    /// Parse the element starting at cursor position (point).
    /// https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L3833
    /// (defun org-element--current-element (limit &optional granularity mode structure)
    ///
    /// LIMIT bounds the search.
    ///
    /// GRANULARITY determines the depth of the
    /// recursion. When it is broader than `object',
    /// secondary values will not be parsed, since they only
    /// contain objects.
    ///
    /// If STRUCTURE isn't provided but MODE is set to `item', it will be
    /// computed.
    ///
    /// This function assumes cursor is always at the beginning of the
    /// element it has to parse."
    fn current_element(
        &self,
        limit: usize,
        mode: ParserMode,
        structure: Option<Rc<ListStruct>>,
    ) -> SyntaxNode<'a> {
        let pos = self.cursor.borrow().pos();

        let raw_secondary_p = self.granularity == ParseGranularity::Object;

        let get_current_element = || -> SyntaxNode<'a> {
            use crate::parser::ParserMode::*;

            // Item
            if mode == Item {
                return self.item_parser(structure, raw_secondary_p);
            }

            // Table Row.
            if mode == TableRow {
                return self.table_row_parser();
            }

            // Node Property.
            if mode == NodeProperty {
                return self.node_property_parser(limit);
            }

            // Headline.
            if self.cursor.borrow_mut().on_headline() {
                return self.headline_parser();
            }

            // Sections (must be checked after headline).
            if mode == Section {
                return self.section_parser(limit);
            }

            if mode == FirstSection {
                let pos = self.cursor.borrow().pos();
                let lim = self.cursor.borrow_mut().next_headline().unwrap_or(limit);
                self.cursor.borrow_mut().set(pos);
                return self.section_parser(lim);
            }

            // Planning.
            {
                let mut c = self.cursor.borrow_mut();
                let maybe_headline_offset = c.line_beginning_position(Some(0));
                let maybe_star = c.char_after(maybe_headline_offset);
                let is_prev_line_headline = Some('*') == maybe_star;
                let is_match_planning = c.looking_at(&*REGEX_PLANNING_LINE).is_some();
                drop(c);

                if mode == Planning && is_prev_line_headline && is_match_planning {
                    return self.planning_parser(limit);
                }
            }

            // Property drawer.
            {
                let mut c = self.cursor.borrow_mut();
                let delta = if mode == Planning { 0 } else { -1 };
                let maybe_headline_offset = c.line_beginning_position(Some(delta));
                let maybe_star = c.char_after(maybe_headline_offset);
                let is_prev_line_headline = Some('*') == maybe_star;

                let is_match_property_drawer = c.looking_at(&*REGEX_PROPERTY_DRAWER).is_some();
                drop(c);

                if (mode == Planning || mode == PropertyDrawer)
                    && is_prev_line_headline
                    && is_match_property_drawer
                {
                    return self.property_drawer_parser(limit);
                }
            }

            // When not at bol, point is at the beginning of an item or
            // a footnote definition: next item is always a paragraph.
            if !self.cursor.borrow().is_bol() {
                return self.paragraph_parser(limit, self.cursor.borrow().pos(), None);
            }

            // Clock.
            if looking_at!(REGEX_CLOCK_LINE, self).is_some() {
                return self.clock_line_parser(limit);
            }

            // Inlinetask.
            if self.cursor.borrow_mut().on_headline() {
                return self.inlinetask_parser(limit, raw_secondary_p);
            }

            // From there, elements can have affiliated keywords.
            let (aff_start, affiliated) = self.collect_affiliated_keywords(limit);

            // If parsing affiliated keywords left cursor off-limits
            // then parse them as regular keywords.
            if (affiliated.is_some() && self.cursor.borrow().pos() >= limit) {
                self.cursor.borrow_mut().set(aff_start);
                return self.keyword_parser(limit, aff_start, None);
            }

            // LaTeX Environment
            //org-element--latex-begin-environment
            if looking_at!(REGEX_LATEX_BEGIN_ENVIRIONMENT, self).is_some() {
                return self.latex_environment_parser(limit, aff_start, affiliated);
            }

            // Drawer and Property Drawer.
            if looking_at!(REGEX_DRAWER, self).is_some() {
                return self.drawer_parser(limit, aff_start, affiliated);
            }

            //  Fixed Width
            if looking_at!(REGEX_FIXED_WIDTH, self).is_some() {
                return self.fixed_width_parser(limit, aff_start, affiliated);
            }

            // Inline Comments, Blocks, Babel Calls, Dynamic Blocks and Keywords.
            //
            // NOTE: We extract match data into owned values before
            // re-borrowing the cursor to avoid RefCell double-borrow panics
            // (the `if let` temporary lifetime extends to the block end).
            let hashtag_end = looking_at!(REGEX_STARTS_WITH_HASHTAG, self).map(|m| m.end());
            if let Some(end) = hashtag_end {
                self.cursor.borrow_mut().set(pos + end);
                if looking_at!(REGEX_COLON_OR_EOL, self).is_some() {
                    self.cursor.borrow_mut().goto_line_begin();
                    return self.comment_parser(limit, aff_start, affiliated);
                }

                let block_name = capturing_at!(REGEX_BLOCK_BEGIN, self)
                    .and_then(|cap| cap.get(1).map(|m| m.as_str().to_ascii_uppercase()));
                if let Some(name) = block_name {
                    self.cursor.borrow_mut().goto_line_begin();
                    match name.as_ref() {
                        "CENTER" => return self.center_block_parser(limit, aff_start, affiliated),
                        "COMMENT" => {
                            return self.comment_block_parser(limit, aff_start, affiliated);
                        }
                        "EXAMPLE" => {
                            return self.example_block_parser(limit, aff_start, affiliated);
                        }
                        "EXPORT" => return self.export_block_parser(limit, aff_start, affiliated),
                        "QUOTE" => return self.quote_block_parser(limit, aff_start, affiliated),
                        "SRC" => return self.src_block_parser(limit, aff_start, affiliated),
                        "VERSE" => return self.verse_block_parser(limit, aff_start, affiliated),
                        _ => return self.special_block_parser(limit, aff_start, affiliated),
                    }
                }

                if looking_at!(REGEX_BABEL_CALL, self).is_some() {
                    self.cursor.borrow_mut().goto_line_begin();
                    return self.babel_call_parser(limit, aff_start, affiliated);
                }

                if looking_at!(REGEX_DYNAMIC_BLOCK, self).is_some() {
                    self.cursor.borrow_mut().goto_line_begin();
                    return self.dynamic_block_parser(limit, aff_start, affiliated);
                }

                if looking_at!(REGEX_KEYWORD, self).is_some() {
                    self.cursor.borrow_mut().goto_line_begin();
                    return self.keyword_parser(limit, aff_start, affiliated);
                }

                // If none of the above fits then this is just a paragraph
                self.cursor.borrow_mut().goto_line_begin();
                return self.paragraph_parser(limit, aff_start, affiliated);
            }

            // Footnote Definition
            if looking_at!(REGEX_FOOTNOTE_DEFINITION, self).is_some() {
                return self.footnote_definition_parser(limit, aff_start, affiliated);
            }

            //  Horizontal Rule.
            if looking_at!(REGEX_HORIZONTAL_RULE, self).is_some() {
                return self.horizontal_rule_parser(limit, aff_start, affiliated);
            }

            // Diary Sexp.
            if looking_at!(REGEX_DIARY_SEXP, self).is_some() {
                return self.diary_sexp_parser(limit, aff_start, affiliated);
            }

            // Table
            // NB: table.el style tables are not supported
            if looking_at!(REGEX_TABLE_BORDER, self).is_some() {
                return self.table_parser(limit, aff_start, affiliated);
            }

            // List.
            //  ((looking-at (org-item-re))
            //   (org-element-plain-list-parser
            //    limit affiliated
            //    (or structure (org-element--list-struct limit))))
            if looking_at!(REGEX_ITEM, self).is_some() {
                let s = structure.unwrap_or(self.list_struct(limit));
                return self.plain_list_parser(limit, aff_start, affiliated, s.clone());
            }

            // Default element: Paragraph.
            return self.paragraph_parser(limit, aff_start, affiliated);
        };

        let current_element = get_current_element();
        self.cursor.borrow_mut().set(pos);
        return current_element;
    }

    /// Parse objects between `beg` and `end` and return recursive structure.
    /// https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L4515
    ///
    /// Objects are accumulated in ACC.  RESTRICTION is a list of object
    /// successors which are allowed in the current object.
    ///
    /// ACC becomes the parent for all parsed objects.  However, if ACC
    /// is nil (i.e., a secondary string is being parsed) and optional
    /// argument PARENT is non-nil, use it as the parent for all objects.
    /// Eventually, if both ACC and PARENT are nil, the common parent is
    /// the list of objects itself."
    /// (defun org-element--parse-objects (beg end acc restriction &optional parent)
    pub fn parse_objects(
        &self,
        beg: usize,
        end: usize,
        restriction: impl Fn(SyntaxT) -> bool,
    ) -> Vec<Rc<SyntaxNode<'a>>> {
        let mut children: Vec<Rc<SyntaxNode<'a>>> = Vec::new();
        let mut pos = beg;

        while pos < end {
            let remaining = &self.input[pos..end];

            // Try to parse each object type in order of precedence
            if let Some((node, consumed)) = self.try_parse_bold(remaining, pos) {
                if restriction(SyntaxT::Bold) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_italic(remaining, pos) {
                if restriction(SyntaxT::Italic) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_code(remaining, pos) {
                if restriction(SyntaxT::Code) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_verbatim(remaining, pos) {
                if restriction(SyntaxT::Verbatim) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_underline(remaining, pos) {
                if restriction(SyntaxT::Underline) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_strikethrough(remaining, pos) {
                if restriction(SyntaxT::StrikeThrough) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_footnote_reference(remaining, pos) {
                if restriction(SyntaxT::FootnoteReference) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_timestamp(remaining, pos) {
                if restriction(SyntaxT::Timestamp) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_entity(remaining, pos) {
                if restriction(SyntaxT::Entity) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_link(remaining, pos) {
                if restriction(SyntaxT::Link) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_target(remaining, pos) {
                if restriction(SyntaxT::Target) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_plain_link(remaining, pos) {
                if restriction(SyntaxT::Link) {
                    children.push(node);
                }
                pos += consumed;
            } else if let Some((node, consumed)) = self.try_parse_plain_text(remaining, pos, end) {
                children.push(node);
                pos += consumed;
            } else {
                // Skip one character and continue
                pos += 1;
            }
        }

        children
    }

    fn is_pre_char(b: u8) -> bool {
        matches!(b, b' ' | b'\t' | b'\n' | b'(' | b'{' | b'\'' | b'"' | b'-')
    }

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
                | b')'
                | b'}'
                | b'\\'
                | b'['
                | b'-'
        )
    }

    fn try_parse_bold<'b>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        self.parse_emphasis_marker(text, start, b'*', SyntaxT::Bold)
    }

    fn try_parse_italic<'b>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        self.parse_emphasis_marker(text, start, b'/', SyntaxT::Italic)
    }

    fn try_parse_underline<'b>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        self.parse_emphasis_marker(text, start, b'_', SyntaxT::Underline)
    }

    fn try_parse_strikethrough<'b>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        self.parse_emphasis_marker(text, start, b'+', SyntaxT::StrikeThrough)
    }

    fn try_parse_code<'b>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        self.parse_emphasis_marker(text, start, b'~', SyntaxT::Code)
    }

    fn try_parse_verbatim<'b>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        self.parse_emphasis_marker(text, start, b'=', SyntaxT::Verbatim)
    }

    fn try_parse_link<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 4 || &bytes[0..2] != b"[[" {
            return None;
        }

        // Find closing ]]
        let mut found_close = None;
        for i in 2..bytes.len() - 1 {
            if &bytes[i..i + 2] == b"]]" {
                let after = i + 2;
                let valid_post = after >= bytes.len() || Self::is_post_char(bytes[after]);
                if valid_post {
                    found_close = Some(i + 2);
                    break;
                }
            }
        }

        let close = found_close?;
        let content_start = 2;
        let content_end = close - 2;
        let raw = &text[..close];

        // Use LinkData constructor - it handles basic link parsing
        let link_data = LinkData::new(raw);

        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Link(Box::new(link_data)),
            location: Interval {
                start,
                end: start + close,
            },
            content_location: Some(Interval {
                start: start + content_start,
                end: start + content_end,
            }),
            post_blank: 0,
            affiliated: None,
        };

        Some((Rc::new(node), close))
    }

    fn try_parse_target<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        let bytes = text.as_bytes();
        if bytes.len() < 4 || &bytes[0..2] != b"<<" {
            return None;
        }

        // Find closing >>
        let mut found_close = None;
        for i in 2..bytes.len() - 1 {
            if &bytes[i..i + 2] == b">>" {
                let after = i + 2;
                let valid_post = after >= bytes.len() || Self::is_post_char(bytes[after]);
                if valid_post {
                    found_close = Some(i + 2);
                    break;
                }
            }
        }

        let close = found_close?;
        let content = &text[2..close - 2];

        let target_data = TargetData::new(content);
        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Target(Box::new(target_data)),
            location: Interval {
                start,
                end: start + close,
            },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        };

        Some((Rc::new(node), close))
    }

    fn parse_emphasis_marker<'b>(
        &self,
        text: &'b str,
        start: usize,
        marker: u8,
        syntax: SyntaxT,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
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

        // Search for closing marker
        for i in 2..text.len() {
            if bytes[i] == b'\n' {
                newlines += 1;
                if newlines > 1 {
                    break;
                }
            }
            if bytes[i] == marker {
                // Last content char must be non-whitespace
                if i >= 2 {
                    let prev = bytes[i - 1];
                    if prev != b' ' && prev != b'\t' && prev != b'\n' {
                        // Check POST condition
                        let valid_post = i + 1 >= text.len() || Self::is_post_char(bytes[i + 1]);
                        if valid_post {
                            found_close = Some(i);
                            break;
                        }
                    }
                }
            }
        }

        let close = found_close?;

        // Extract content between markers
        let content_start = start + 1;
        let content_end = start + close;
        let content = &self.input[content_start..content_end];

        // Parse inner objects recursively
        let inner_children = self.parse_objects(content_start, content_end, |_| true);

        // Create the node based on syntax type
        let node = match syntax {
            SyntaxT::Bold => SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(inner_children),
                data: Syntax::Bold,
                location: Interval {
                    start,
                    end: start + close + 1,
                },
                content_location: Some(Interval {
                    start: content_start,
                    end: content_end,
                }),
                post_blank: 0,
                affiliated: None,
            },
            SyntaxT::Italic => SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(inner_children),
                data: Syntax::Italic,
                location: Interval {
                    start,
                    end: start + close + 1,
                },
                content_location: Some(Interval {
                    start: content_start,
                    end: content_end,
                }),
                post_blank: 0,
                affiliated: None,
            },
            SyntaxT::Underline => SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(inner_children),
                data: Syntax::Underline,
                location: Interval {
                    start,
                    end: start + close + 1,
                },
                content_location: Some(Interval {
                    start: content_start,
                    end: content_end,
                }),
                post_blank: 0,
                affiliated: None,
            },
            SyntaxT::StrikeThrough => SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(inner_children),
                data: Syntax::StrikeThrough,
                location: Interval {
                    start,
                    end: start + close + 1,
                },
                content_location: Some(Interval {
                    start: content_start,
                    end: content_end,
                }),
                post_blank: 0,
                affiliated: None,
            },
            SyntaxT::Code => SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(vec![]),
                data: Syntax::Code(Box::new(CodeData { value: content })),
                location: Interval {
                    start,
                    end: start + close + 1,
                },
                content_location: Some(Interval {
                    start: content_start,
                    end: content_end,
                }),
                post_blank: 0,
                affiliated: None,
            },
            SyntaxT::Verbatim => SyntaxNode {
                parent: RefCell::new(None),
                children: RefCell::new(vec![]),
                data: Syntax::Verbatim(Box::new(VerbatimData { value: content })),
                location: Interval {
                    start,
                    end: start + close + 1,
                },
                content_location: Some(Interval {
                    start: content_start,
                    end: content_end,
                }),
                post_blank: 0,
                affiliated: None,
            },
            _ => return None,
        };

        Some((Rc::new(node), close + 1))
    }

    fn try_parse_plain_link<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        const PROTOCOLS: &[&[u8]] = &[b"https://", b"http://", b"ftp://", b"mailto:"];
        let bytes = text.as_bytes();
        let proto_len = PROTOCOLS.iter().find_map(|&p| {
            if bytes.starts_with(p) { Some(p.len()) } else { None }
        })?;
        let url_end = text[proto_len..]
            .find(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | '<' | '>'))
            .map_or(text.len(), |i| proto_len + i);
        if url_end <= proto_len {
            return None;
        }
        let raw = &text[..url_end];
        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Link(Box::new(LinkData::new_plain(raw))),
            location: Interval { start, end: start + url_end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        };
        Some((Rc::new(node), url_end))
    }

    fn try_parse_footnote_reference<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        if !text.starts_with("[fn:") {
            return None;
        }
        let close = text.find(']')?;
        let inner = &text[4..close];
        let (label, type_s): (Option<&'a str>, &'a str) = if let Some(colon_pos) = inner.find(':') {
            let label_part = &inner[..colon_pos];
            (if label_part.is_empty() { None } else { Some(label_part) }, "inline")
        } else {
            (if inner.is_empty() { None } else { Some(inner) }, "standard")
        };
        let consumed = close + 1;
        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::FootnoteReference(Box::new(FootnoteReferenceData { label, type_s })),
            location: Interval { start, end: start + consumed },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        };
        Some((Rc::new(node), consumed))
    }

    fn try_parse_plain_text<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
        _end: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        if text.is_empty() {
            return None;
        }

        // Find where plain text ends (at next markup marker or end)
        let bytes = text.as_bytes();
        let mut consume = 0;

        for (i, &b) in bytes.iter().enumerate() {
            // Stop at emphasis markers that could start markup
            if matches!(b, b'*' | b'/' | b'_' | b'+' | b'=' | b'~') && i > 0 {
                // Check if it's a valid PRE char for markup
                if Self::is_pre_char(bytes[i - 1]) {
                    break;
                }
            }
            // Stop before '[' so footnote references, links, and timestamps get a chance
            if b == b'[' {
                break;
            }
            // Stop before bare URL protocols at word boundaries
            if i == 0 || Self::is_pre_char(bytes[i - 1]) {
                let rem = &bytes[i..];
                if rem.starts_with(b"https://")
                    || rem.starts_with(b"http://")
                    || rem.starts_with(b"ftp://")
                    || rem.starts_with(b"mailto:")
                {
                    break;
                }
            }
            consume = i + 1;
        }

        if consume == 0 {
            return None;
        }

        let content = &text[..consume];
        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::PlainText(content),
            location: Interval {
                start,
                end: start + consume,
            },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        };

        Some((Rc::new(node), consume))
    }

    fn try_parse_timestamp<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || (bytes[0] != b'<' && bytes[0] != b'[') {
            return None;
        }

        let closing = if bytes[0] == b'<' { b'>' } else { b']' };

        // Find closing bracket
        let mut found_close = None;
        for i in 1..text.len() {
            if bytes[i] == closing {
                // Check for space or end after closing
                if i + 1 >= text.len() || Self::is_post_char(bytes[i + 1]) {
                    found_close = Some(i);
                    break;
                }
            }
        }

        let close = found_close?;

        // Extract content between brackets
        let content = &text[1..close];
        let raw = &text[..close + 1];

        // Try to create a valid TimestampData
        let timestamp_data = TimestampData::new(raw)?;

        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Timestamp(Box::new(timestamp_data)),
            location: Interval {
                start,
                end: start + close + 1,
            },
            content_location: Some(Interval {
                start: start + 1,
                end: start + close,
            }),
            post_blank: 0,
            affiliated: None,
        };

        Some((Rc::new(node), close + 1))
    }

    fn try_parse_entity<'b: 'a>(
        &self,
        text: &'b str,
        start: usize,
    ) -> Option<(Rc<SyntaxNode<'a>>, usize)> {
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

        let node = SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Entity(Box::new(entity_data)),
            location: Interval {
                start,
                end: start + end,
            },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        };

        Some((Rc::new(node), end))
    }
}
