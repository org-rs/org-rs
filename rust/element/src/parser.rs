extern crate xi_rope;

use crate::data::Handle;
use crate::data::SyntaxT;
use crate::data::{Syntax, SyntaxNode};
use crate::headline::REGEX_HEADLINE_SHORT;
use std::cell::RefCell;
use std::rc::Rc;

use xi_rope::find::find;
use xi_rope::find::CaseMatching::CaseInsensitive;
use xi_rope::tree::Node;
use xi_rope::RopeInfo;
use xi_rope::{Cursor, LinesMetric};

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
#[derive(Copy, Clone)]
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
    cursor: RefCell<Cursor<'a, RopeInfo>>,
    input: &'a Node<RopeInfo>,
    granularity: ParseGranularity,
}

impl<'a> Parser<'a> {
    fn new(input: &'a Node<RopeInfo>, granularity: ParseGranularity) -> Parser {
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
        if self.granularity == ParseGranularity::Headline && !self.at_headline() {
            self.next_headline();
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
                if element.data.is_greater_element() {
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
                // Any other element with contents, if granularity allows it
                else {
                    // (org-element--parse-objects
                    //    cbeg (org-element-property :contents-end element)
                    //    element (org-element-restriction type))))
                    if let ParseGranularity::Object = &self.granularity {
                        element.children.replace(self.parse_objects(
                            content_location.start,
                            content_location.end,
                            |that| element.data.can_contain(that),
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
    ///
    /// Return value is a list like (TYPE PROPS) where TYPE is the type
    /// of the element and PROPS a plist of properties associated to the
    /// element.
    ///
    /// Possible types are defined in `org-element-all-elements'.
    ///
    /// LIMIT bounds the search.
    ///
    /// Optional argument GRANULARITY determines the depth of the
    /// recursion.  Allowed values are `headline', `greater-element',
    /// `element', `object' or nil.  When it is broader than `object' (or
    /// nil), secondary values will not be parsed, since they only
    /// contain objects.
    ///
    /// Optional argument MODE, when non-nil, can be either
    /// `first-section', `section', `planning', `item', `node-property'
    /// and `table-row'.
    ///
    /// If STRUCTURE isn't provided but MODE is set to `item', it will be
    /// computed.
    ///
    /// This function assumes point is always at the beginning of the
    /// element it has to parse."
    ///
    /// (defun org-element--current-element (limit &optional granularity mode structure)
    fn current_element(
        &self,
        limit: usize,
        mode: ParserMode,
        structure: Option<&ListStruct>,
    ) -> SyntaxNode<'a> {
        let pos = self.cursor.borrow().pos();
        // TODO write current_element function #9
        let raw_secondary_p = self.granularity == ParseGranularity::Object;

        let current_element = match mode {
            // Item
            // ((eq mode 'item)
            //(org-element-item-parser limit structure raw-secondary-p))
            ParserMode::Item => self.item_parser(structure, raw_secondary_p),

            // Table Row.
            // ((eq mode 'table-row) (org-element-table-row-parser limit))
            ParserMode::TableRow => self.table_row_parser(),

            // Node Property.
            // ((eq mode 'node-property) (org-element-node-property-parser limit))
            ParserMode::NodeProperty => self.node_property_parser(limit),

            // Headline.
            // ((org-with-limited-levels (org-at-heading-p))
            //  (org-element-headline-parser limit raw-secondary-p))
            _ => unimplemented!(),
            //  ;; Sections (must be checked after headline).
            //  ((eq mode 'section) (org-element-section-parser limit))
            //  ((eq mode 'first-section)
            //  (org-element-section-parser
            //      (or (save-excursion (org-with-limited-levels (outline-next-heading)))
            //  limit)))
        };

        self.cursor.borrow_mut().set(pos);
        return current_element;
    }

    /// Checks if current line of the cursor is a headline
    /// In emacs defined as org-at-heading-p which is a proxy to
    /// outline-on-heading-p at outline.el
    fn at_headline(&self) -> bool {
        let pos = self.cursor.borrow().pos();
        let beg = self.cursor.borrow_mut().goto_line_begin();
        self.cursor.borrow_mut().set(pos);
        let mut raw_lines = self.input.lines_raw(beg..self.input.len());
        match raw_lines.next() {
            Some(line) => REGEX_HEADLINE_SHORT.is_match(&line),
            None => false,
        }
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
        restriction: impl Fn(&Syntax) -> bool,
    ) -> Vec<Handle> //acc
    {
        let pos = self.cursor.borrow().pos();
        // TODO write parse_objects function #8
        self.cursor.borrow_mut().set(pos);
        unimplemented!();
    }

    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    fn next_headline(&self) -> Option<(usize)> {
        let pos = self.cursor.borrow().pos();
        let mut raw_lines = self
            .input
            .lines_raw(self.cursor.borrow().pos()..self.input.len());
        // make sure we don't match current headline
        raw_lines.next();
        self.cursor.borrow_mut().next::<LinesMetric>();

        // TODO consider using FULL headline regex and consider leaving cursor at the end of match
        let search = find(
            &mut self.cursor.borrow_mut(),
            &mut raw_lines,
            CaseInsensitive,
            REGEX_HEADLINE_SHORT.as_str(),
            Some(&*REGEX_HEADLINE_SHORT),
        );
        match search {
            None => {
                self.cursor.borrow_mut().set(pos);
                None
            }
            Some(begin) => {
                self.cursor.borrow_mut().set(begin);
                Some(begin)
            }
        }
    }
}

/// Handy things for cursor
pub trait CursorHelper {
    /// Skip over space, tabs and newline characters
    /// Cursor position is set before next non-whitespace char
    fn skip_whitespace(&mut self) -> usize;
    fn goto_line_begin(&mut self) -> usize;
}

impl<'a> CursorHelper for Cursor<'a, RopeInfo> {
    fn skip_whitespace(&mut self) -> usize {
        while let Some(c) = self.next_codepoint() {
            if !(c.is_whitespace()) {
                self.prev_codepoint();
                break;
            } else {
                self.next_codepoint();
            }
        }
        self.pos()
    }

    /// Moves cursor to the beginning of the current line.
    /// If cursor is already at the beginning of the line - nothing happens
    /// Returns the position of the cursor
    fn goto_line_begin(&mut self) -> usize {
        if self.pos() != 0 {
            if self.at_or_prev::<LinesMetric>().is_none() {
                self.set(0);
            }
        }
        self.pos()
    }
}

mod test {
    use crate::data::Syntax;
    use crate::data::Syntax::Section;
    use crate::parser::CursorHelper;
    use crate::parser::Parser;
    use crate::parser::{ParseGranularity, REGEX_HEADLINE_SHORT};
    use core::borrow::Borrow;
    use std::str::FromStr;
    use xi_rope::find::find;
    use xi_rope::find::CaseMatching::CaseInsensitive;
    use xi_rope::{Cursor, Rope};

    #[test]
    fn at_headline() {
        let rope = Rope::from_str("Some text\n**** headline\n").unwrap();
        let parser = Parser::new(&rope, ParseGranularity::Object);
        assert!(!parser.at_headline());
        parser.cursor.borrow_mut().set(4);
        assert!(!parser.at_headline());
        assert_eq!(4, parser.cursor.borrow().pos());
        parser.cursor.borrow_mut().set(15);
        assert!(parser.at_headline());
        assert_eq!(15, parser.cursor.borrow().pos());
    }

    #[test]
    fn next_headline() {
        let rope = Rope::from_str("Some text\n**** headline\n").unwrap();
        let parser = Parser::new(&rope, ParseGranularity::Object);

        assert_eq!(Some(10), parser.next_headline());
        assert_eq!(10, parser.cursor.borrow().pos());

        let rope = Rope::from_str("* First\n** Second\n").unwrap();
        let parser = Parser::new(&rope, ParseGranularity::Object);
        assert_eq!(Some(8), parser.next_headline());
        assert_eq!(8, parser.cursor.borrow().pos());
    }

    #[test]
    fn skip_whitespaces() {
        let rope = Rope::from_str(" \n\t\rorg-mode ").unwrap();
        let mut cursor = Cursor::new(&rope, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.next_codepoint().unwrap(), 'o');

        let rope2 = Rope::from_str("no_whitespace_for_you!").unwrap();
        cursor = Cursor::new(&rope2, 0);
        cursor.skip_whitespace();
        assert_eq!(cursor.next_codepoint().unwrap(), 'n');

        // Skipping all the remaining whitespace results in invalid cursor at the end of the rope
        let rope3 = Rope::from_str(" ").unwrap();
        cursor = Cursor::new(&rope3, 0);
        cursor.skip_whitespace();
        assert_eq!(None, cursor.next_codepoint());
    }

}
