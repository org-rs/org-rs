extern crate xi_rope;

use crate::data::Handle;
use crate::data::SyntaxInfo;
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
    cursor: Cursor<'a, RopeInfo>,
    input: &'a Node<RopeInfo>,
    granularity: ParseGranularity,
}

impl<'a> Parser<'a> {
    fn new(input: &'a Node<RopeInfo>, granularity: ParseGranularity) -> Parser {
        Parser {
            cursor: Cursor::new(input, 0),
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
    fn parse_buffer(&mut self) -> SyntaxNode {
        self.cursor.set(0);
        self.cursor.skip_whitespace();

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
    fn parse_elements(
        &mut self,
        beg: usize,
        end: usize,
        mut mode: ParserMode,
        structure: Option<&ListStruct>,
    ) -> Vec<Handle> {
        let pos = self.cursor.pos();
        self.cursor.set(beg);

        // When parsing only headlines, skip any text before first one.
        if self.granularity == ParseGranularity::Headline && !self.at_headline() {
            self.next_headline();
        }

        let mut elements: Vec<Handle> = vec![];
        while self.cursor.pos() < end {
            // Find current element's type and parse it accordingly to its category.
            // (org-element--current-element end granularity mode structure))
            let element: SyntaxNode = self.current_element(end, mode, structure);

            // (goto-char (org-element-property :end element))
            self.cursor.set(element.location.end);

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
                            restriction, //FIXME pass `can_contain` func of current element
                        ));
                    }
                }
            }
            if let Some(m) = Parser::next_mode(&element.data, false) {
                mode = m
            }
            elements.push(Rc::new(element));
        }
        self.cursor.set(pos);
        elements
    }

    ///   "Parse the element starting at cursor position (point).
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
        &mut self,
        limit: usize,
        mode: ParserMode,
        structure: Option<&ListStruct>,
    ) -> SyntaxNode {
        let pos = self.cursor.pos();

        self.cursor.set(pos);
        unimplemented!();
    }

    /// Checks if current line of the cursor is a headline
    /// In emacs defined as org-at-heading-p which is a proxy to
    /// outline-on-heading-p at outline.el
    fn at_headline(&mut self) -> bool {
        let pos = self.cursor.pos();
        let beg = self.cursor.goto_line_begin();
        self.cursor.set(pos);
        let mut raw_lines = self.input.lines_raw(beg..self.input.len());
        match raw_lines.next() {
            Some(line) => REGEX_HEADLINE_SHORT.is_match(&line),
            None => false,
        }
    }

    // FIXME figure out restriction type
    fn parse_objects(&mut self, beg: usize, end: usize, restriction: ()) -> Vec<Handle> {
        unimplemented!()
    }

    /// Possibly moves cursor to the beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    fn next_headline(&mut self) -> Option<(usize)> {
        let pos = self.cursor.pos();
        let mut raw_lines = self.input.lines_raw(self.cursor.pos()..self.input.len());
        // make sure we don't match current headline
        raw_lines.next();
        self.cursor.next::<LinesMetric>();

        // TODO consider using FULL headline regex and consider leaving cursor at the end of match
        let search = find(
            &mut self.cursor,
            &mut raw_lines,
            CaseInsensitive,
            REGEX_HEADLINE_SHORT.as_str(),
            Some(&*REGEX_HEADLINE_SHORT),
        );
        match search {
            None => {
                self.cursor.set(pos);
                None
            }
            Some(begin) => {
                self.cursor.set(begin);
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
            if let None = self.at_or_prev::<LinesMetric>() {
                self.set(0);
            }
        }
        self.pos()
    }
}

mod test {
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
        let mut parser = Parser::new(&rope, ParseGranularity::Object);
        assert!(!parser.at_headline());
        parser.cursor.set(4);
        assert!(!parser.at_headline());
        assert_eq!(4, parser.cursor.pos());
        parser.cursor.set(15);
        assert!(parser.at_headline());
        assert_eq!(15, parser.cursor.pos());
    }

    #[test]
    fn next_headline() {
        let rope = Rope::from_str("Some text\n**** headline\n").unwrap();
        let mut parser = Parser::new(&rope, ParseGranularity::Object);

        assert_eq!(Some(10), parser.next_headline());
        assert_eq!(10, parser.cursor.pos());

        let rope = Rope::from_str("* First\n** Second\n").unwrap();
        let mut parser = Parser::new(&rope, ParseGranularity::Object);
        assert_eq!(Some(8), parser.next_headline());
        assert_eq!(8, parser.cursor.pos());
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
