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

use crate::cursor::{Cursor, CursorHelper};
use crate::data::Handle;
use crate::data::SyntaxT;
use crate::data::{Syntax, SyntaxNode};
use crate::headline::REGEX_CLOCK_LINE;
use crate::headline::REGEX_HEADLINE_SHORT;
use crate::headline::REGEX_PLANNING_LINE;
use crate::headline::REGEX_PROPERTY_DRAWER;
use crate::list::*;

/// determines the depth of the recursion.
#[derive(PartialEq)]
pub enum ParseGranularity {
    /// Only parse headlines.
    Headline,
    /// Don't recurse into greater elements except
    /// headlines and sections.  Thus, elements
    /// parsed are the top-level ones.
    GreaterElement,
    /// Parse everything but objects and plain text.
    Element,
    /// Parse the complete buffer (default).
    Object,
}

/// MODE prioritizes some elements over the others
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

pub struct Parser<'a> {
    pub cursor: RefCell<Cursor<'a>>,
    pub input: &'a str,
    pub granularity: ParseGranularity,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str, granularity: ParseGranularity) -> Parser {
        Parser {
            cursor: RefCell::new(Cursor::new(input, 0)),
            input,
            granularity,
        }
    }

    /// Returns parser mode according to given `element` and `is_parent`
    /// `element` is AllElements variant representing the type of an element
    /// containing next element if `is_parent` is true, or before it
    /// otherwise.
    /// <br>
    /// Original function name: org-element--next-mode
    /// https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L4273
    /// TODO refactor to SyntaxT
    fn next_mode(syntax: &Syntax, is_parent: bool) -> Option<ParserMode> {
        if is_parent {
            match syntax {
                Syntax::Headline { .. } => Some(ParserMode::Section),
                Syntax::InlineTask { .. } => Some(ParserMode::Planning),
                Syntax::PlainList { .. } => Some(ParserMode::Item),
                Syntax::PropertyDrawer { .. } => Some(ParserMode::NodeProperty),
                Syntax::Section { .. } => Some(ParserMode::Planning),
                Syntax::Table { .. } => Some(ParserMode::TableRow),
                _ => None,
            }
        } else {
            match syntax {
                Syntax::Item { .. } => Some(ParserMode::Item),
                Syntax::NodeProperty { .. } => Some(ParserMode::NodeProperty),
                Syntax::Planning { .. } => Some(ParserMode::PropertyDrawer),
                Syntax::TableRow { .. } => Some(ParserMode::TableRow),
                _ => None,
            }
        }
    }

    /// org-element-parse-buffer
    /// Parses input from beginning to the end
    fn parse_buffer(&'a self) -> SyntaxNode {
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
        structure: Option<&ListStruct>,
    ) -> Vec<Handle> {
        let pos = self.cursor.borrow_mut().pos();
        self.cursor.borrow_mut().set(beg);

        // When parsing only headlines, skip any text before first one.
        if self.granularity == ParseGranularity::Headline && !self.cursor.borrow_mut().on_headline()
        {
            self.cursor.borrow_mut().next_headline();
        }

        let mut elements: Vec<Handle> = vec![];
        loop {
            let current_pos = self.cursor.borrow().pos();
            if current_pos >= end {
                break;
            }

            // Find current element's type and parse it accordingly to its category.
            // (org-element--current-element end granularity mode structure))
            let element: SyntaxNode = self.current_element(end, mode, structure);

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
                            Syntax::PlainList(d) => Some(&d.structure),
                            _ => None,
                        };

                        //  Possibly switch to a special mode.
                        // (org-element--next-mode type t)
                        let new_mode = Parser::next_mode(&element.data, true).unwrap_or(mode);

                        element.children.replace(self.parse_elements(
                            content_location.start,
                            content_location.end,
                            new_mode,
                            list_sturct,
                        ));
                    }
                }
                // ctermfg=37 gui=italic guifg=#2aa1ae
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
            if let Some(m) = Parser::next_mode(&element.data, false) {
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
        structure: Option<&ListStruct>,
    ) -> SyntaxNode<'a> {
        let pos = self.cursor.borrow().pos();

        let raw_secondary_p = self.granularity == ParseGranularity::Object;

        let get_current_element = || -> SyntaxNode<'a> {
            use crate::parser::ParserMode::*;

            // Item
            // ((eq mode 'item)
            //(org-element-item-parser limit structure raw-secondary-p))
            if mode == Item {
                return self.item_parser(structure, raw_secondary_p);
            }

            // Table Row.
            // ((eq mode 'table-row) (org-element-table-row-parser limit))
            if mode == TableRow {
                return self.table_row_parser();
            }

            // Node Property.
            // ((eq mode 'node-property) (org-element-node-property-parser limit))
            if mode == NodeProperty {
                return self.node_property_parser(limit);
            }

            // Headline.
            // ((org-with-limited-levels (org-at-heading-p))
            //  (org-element-headline-parser limit raw-secondary-p))
            if self.cursor.borrow_mut().on_headline() {
                return self.headline_parser();
            }

            // Sections (must be checked after headline).

            // ((eq mode 'section) (org-element-section-parser limit))
            if mode == Section {
                return self.section_parser(limit);
            }

            //  ((eq mode 'first-section)
            //  (org-element-section-parser
            //      (or (save-excursion (org-with-limited-levels (outline-next-heading)))
            //  limit)))
            if mode == FirstSection {
                let pos = self.cursor.borrow().pos();
                let lim = self.cursor.borrow_mut().next_headline().unwrap_or(limit);
                self.cursor.borrow_mut().set(pos);
                return self.section_parser(lim);
            }

            // Planning.
            // ((and (eq mode 'planning)
            //   (eq ?* (char-after (line-beginning-position 0)))
            //   (looking-at org-planning-line-re))
            //  (org-element-planning-parser limit))
            {
                let mut c = self.cursor.borrow_mut();
                let maybe_headline_offset = c.line_beginning_position(Some(0));
                let maybe_star = c.char_after(maybe_headline_offset);
                let is_prev_line_headline = Some('*') == maybe_star;
                let is_match_planning = c.looking_at(&*REGEX_PLANNING_LINE);
                drop(c);

                if mode == Planning && is_prev_line_headline && is_match_planning {
                    return self.planning_parser(limit);
                }
            }

            // Property drawer.
            //     ((and (memq mode '(planning property-drawer))
            // (eq ?* (char-after (line-beginning-position
            //     (if (eq mode 'planning) 0 -1))))
            // (looking-at org-property-drawer-re))
            // (org-element-property-drawer-parser limit))
            {
                let mut c = self.cursor.borrow_mut();
                let delta = if mode == Planning { 0 } else { -1 };
                let maybe_headline_offset = c.line_beginning_position(Some(delta));
                let maybe_star = c.char_after(maybe_headline_offset);
                let is_prev_line_headline = Some('*') == maybe_star;

                let is_match_property_drawer = c.looking_at(&*REGEX_PROPERTY_DRAWER);
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
            // ((not (bolp)) (org-element-paragraph-parser limit (list (point))))
            if !self.cursor.borrow().is_bol() {
                return self.paragraph_parser(limit, self.cursor.borrow().pos());
            }

            // Clock.
            // ((looking-at org-clock-line-re) (org-element-clock-parser limit))
            if self.cursor.borrow_mut().looking_at(&*REGEX_CLOCK_LINE) {
                return self.clock_line_parser(limit);
            }

            // Inlinetask.
            // ((org-at-heading-p)
            //   (org-element-inlinetask-parser limit raw-secondary-p))
            if self.cursor.borrow_mut().on_headline() {
                return self.inlinetask_parser(limit, raw_secondary_p);
            }

            // From there, elements can have affiliated keywords.
            // TODO finish current_element fn

            return unreachable!();
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
    fn parse_objects(
        &self,
        beg: usize,
        end: usize,
        restriction: impl Fn(SyntaxT) -> bool,
    ) -> Vec<Handle> //acc
    {
        let pos = self.cursor.borrow().pos();
        // TODO write parse_objects function #8
        self.cursor.borrow_mut().set(pos);
        unimplemented!();
    }
}
