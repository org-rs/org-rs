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

//!  Plain Lists and Items
//! https://orgmode.org/worg/dev/org-syntax.html#Plain_Lists_and_Items
//!
//!  Items are defined by a line starting with the following pattern: “BULLET
//! COUNTER-SET CHECK-BOX TAG”, in which only BULLET is mandatory.
//!
//!  BULLET is either an asterisk, a hyphen, a plus sign character or follows
//! either the pattern “COUNTER.” or “COUNTER)”.  In any case, BULLET is follwed by
//! a whitespace character or line ending.
//!
//!  COUNTER can be a number or a single letter.
//!
//!  COUNTER-SET follows the pattern [@COUNTER].
//!
//!  CHECK-BOX is either a single whitespace character, a “X” character or a
//! hyphen, enclosed within square brackets.
//!
//!  TAG follows “TAG-TEXT ::” pattern, where TAG-TEXT can contain any character
//! but a new line.
//!
//!  An item ends before the next item, the first line less or equally indented
//! than its starting line, or two consecutive empty lines. Indentation of lines
//! within other greater elements do not count, neither do inlinetasks boundaries.
//!
//!  A plain list is a set of consecutive items of the same indentation. It can
//! only directly contain items.
//!
//!  If first item in a plain list has a counter in its bullet, the plain list will
//! be an “ordered plain-list”. If it contains a tag, it will be a “descriptive
//! list”. Otherwise, it will be an “unordered list”. List types are mutually
//! exclusive.
//!
//!  For example, consider the following excerpt of an Org document:
//!
//!  1. item 1
//!  2. [X] item 2
//!     - some tag :: item 2.1
//!
//!
//!  Its internal structure is as follows:
//!
//!
//!  (ordered-plain-list
//!   (item)
//!   (item
//!    (descriptive-plain-list
//!     (item))))
//!

use std::rc::Rc;

use crate::affiliated::ElementSpan;
use crate::data::{Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use memchr::memchr;
use regex::Regex;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;

lazy_static! {

//TODO implement all regexes
// (defconst org-list-end-re "^[ \t]*\n[ \t]*\n"
//   "Regex matching the end of a plain list.")
//
// (defconst org-list-full-item-re
//   (concat "^[ \t]*\\(\\(?:[-+*]\\|\\(?:[0-9]+\\|[A-Za-z]\\)[.)]\\)\\(?:[ \t]+\\|$\\)\\)"
// 	  "\\(?:\\[@\\(?:start:\\)?\\([0-9]+\\|[A-Za-z]\\)\\][ \t]*\\)?"
// 	  "\\(?:\\(\\[[ X-]\\]\\)\\(?:[ \t]+\\|$\\)\\)?"
// 	  "\\(?:\\(.*\\)[ \t]+::\\(?:[ \t]+\\|$\\)\\)?")
//   "Matches a list item and puts everything into groups:
// group 1: bullet
// group 2: counter
// group 3: checkbox
// group 4: description tag")
//
// (defun org-item-re ()
//   "Return the correct regular expression for plain lists."
//   (let ((term (cond
// 	       ((eq org-plain-list-ordered-item-terminator t) "[.)]")
// 	       ((=  org-plain-list-ordered-item-terminator ?\)) ")")
// 	       ((=  org-plain-list-ordered-item-terminator ?.) "\\.")
// 	       (t "[.)]")))
// 	(alpha (if org-list-allow-alphabetical "\\|[A-Za-z]" "")))
//     (concat "\\([ \t]*\\([-+]\\|\\(\\([0-9]+" alpha "\\)" term
// 	    "\\)\\)\\|[ \t]+\\*\\)\\([ \t]+\\|$\\)")))
//
// (defsubst org-item-beginning-re ()
//   "Regexp matching the beginning of a plain list item."
//   (concat "^" (org-item-re)))

    pub static ref REGEX_ITEM : Regex = Regex::new(r"([ \t]*([-+]|(([0-9]+)[.)]))|[ \t]+\*)([ \t]|$)").unwrap();

}

/// List structure - tracks items during list parsing
/// Used to compute list boundaries and parent-child relationships
#[derive(Debug, Clone)]
pub struct ListStruct<'a> {
    /// Items found so far: (position, indent, bullet, counter, checkbox, tag)
    pub items: Vec<ListItem<'a>>,
}

#[derive(Debug, Clone)]
pub struct ListItem<'a> {
    pub position: usize,
    pub indent: usize,
    pub bullet: &'a str,
    pub counter: Option<usize>,
    pub checkbox: Option<CheckBox>,
    pub tag: Option<&'a str>,
}

impl<'a> ListStruct<'a> {
    pub fn new() -> Self {
        ListStruct { items: Vec::new() }
    }
}

#[derive(Debug)]
pub struct ItemData<'rope> {
    /// Item's bullet (string).
    bullet: Cow<'rope, str>,
    /// Item's check_box, if any (symbol on, off, trans, nil).
    checkbox: Option<CheckBox>,
    /// Item's counter, if any. Literal counters become ordinals (integer).
    pub counter: usize,
    /// Number of newline characters between the beginning
    /// of the item and the beginning of the contents (0, 1 or 2).
    pre_blank: usize,
    /// Uninterpreted item's tag, if any (string or nil).
    raw_tag: Option<Cow<'rope, str>>,
    /// Parsed item's tag, if any (secondary string or nil).
    tag: Option<Cow<'rope, str>>,
    // TODO figure out what is list structure
    // /// Full list's structure, as returned by org_list_struct (alist).
    structure: ListStruct<'rope>,
}

#[derive(Debug)]
pub struct PlainListData<'a> {
    /// Full list's structure, as returned by org_list_struct (alist).
    pub structure: Rc<ListStruct<'a>>,

    ///List's type (symbol descriptive, ordered, unordered).
    pub type_s: ListKind,
}

#[derive(Debug)]
#[repr(u8)]
pub enum ListKind {
    Descriptive,
    Ordered,
    Unordered,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum CheckBox {
    On,
    Off,
    Trans,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Fallback: item parser (not yet fully implemented).
    pub fn item_parser(
        &mut self,
        _structure: Option<Rc<ListStruct<'a>>>,
        _raw_secondary_p: bool,
    ) -> NodeId {
        let start = self.cursor.pos();
        self.arena.alloc(SyntaxNode::fallback(self.input, start, self.input.len()))
    }

    /// Fallback: plain list parser (not yet fully implemented).
    pub fn plain_list_parser(
        &mut self,
        element_span: ElementSpan<'a>,
        structure: Rc<ListStruct<'a>>,
    ) -> NodeId {
        let span = element_span.span;
        let items = &structure.items;
        if items.is_empty() {
            return self.arena.alloc(SyntaxNode::fallback(self.input, span.start, span.end));
        }

        let first_indent = items[0].indent;
        let list_type = Self::get_list_type(items);

        let mut children = Vec::new();
        let mut i = 0;

        while i < items.len() {
            let item = &items[i];
            if item.indent != first_indent {
                break;
            }

            let end_pos = if i + 1 < items.len() && items[i + 1].indent == first_indent {
                items[i + 1].position
            } else {
                span.end
            };

            let item_node = self.item_parser_internal(item, end_pos);
            children.push(item_node);
            i += 1;
        }

        let end = if let Some(last) = children.last() {
            self.arena[*last].location.end
        } else {
            span.end
        };

        // Recurse into item content at Element/Object granularity.
        // Item is a greater element but its children are pre-built here,
        // so the standard content_location recursion path never fires for them.
        use crate::parser::{ParseGranularity, ParserMode};
        if matches!(self.granularity, ParseGranularity::Element | ParseGranularity::Object) {
            for &item_rc in &children {
                if let Some(loc) = self.arena[item_rc].content_location {
                    let item_children =
                        self.parse_elements(loc, ParserMode::Planning, None);
                    self.arena.set_children(item_rc, item_children);
                }
            }
        }

        let list_data = PlainListData {
            structure: structure.clone(),
            type_s: list_type,
        };

        self.arena.alloc_with_children(SyntaxNode::new(Syntax::PlainList(Box::new(list_data)), (span.start, end))
            .affiliated(element_span.affiliated)
            .build(), children)
    }

    fn get_list_type(items: &[ListItem<'a>]) -> ListKind {
        if items.is_empty() {
            return ListKind::Unordered;
        }
        if items[0].tag.is_some() {
            return ListKind::Descriptive;
        }
        for item in items {
            if item
                .bullet
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                return ListKind::Ordered;
            }
        }
        ListKind::Unordered
    }

    fn item_parser_internal(&mut self, item: &ListItem<'a>, end: usize) -> NodeId {
        let bullet = Cow::Borrowed(item.bullet);
        let tag = item.tag.map(Cow::Borrowed);

        // Content begins after the bullet and its trailing space/tab.
        let content_start = item.position + item.indent + item.bullet.len() + 1;
        let content_location = (content_start < end)
            .then_some(Interval { start: content_start, end });

        let item_data = ItemData {
            bullet,
            checkbox: item.checkbox.clone(),
            counter: item.counter.unwrap_or(0),
            pre_blank: 0,
            raw_tag: tag.clone(),
            tag,
            structure: ListStruct::new(),
        };

        self.arena.alloc(SyntaxNode {
            parent: None,
            children: Vec::new(),
            data: Syntax::Item(Box::new(item_data)),
            location: Interval {
                start: item.position,
                end,
            },
            content_location,
            post_blank: 0,
            affiliated: None,
        })
    }

    /// Scan input for list items at current indentation level
    /// This matches Elisp's org-element--list-struct
    pub fn list_struct(&self, limit: usize) -> Rc<ListStruct<'a>> {
        let mut items = Vec::new();
        let mut pos = self.cursor.pos();
        let input = self.input;

        while pos < limit {
            let (line, next_pos) = match memchr(b'\n', input[pos..].as_bytes()) {
                Some(nl_pos) => (&input[pos..pos + nl_pos], pos + nl_pos + 1),
                None => (&input[pos..], limit),
            };

            let (indent, rest) = Self::get_indent(line);
            if rest.is_empty() {
                pos = next_pos;
                continue;
            }

            let starts_with_ordered = {
                let bs = rest.as_bytes();
                let mut j = 0;
                while j < bs.len() && bs[j].is_ascii_digit() {
                    j += 1;
                }
                j > 0
                    && j < bs.len()
                    && (bs[j] == b'.' || bs[j] == b')')
                    && (j + 1 >= bs.len() || bs[j + 1] == b' ' || bs[j + 1] == b'\t')
            };
            let starts_with_bullet =
                rest.starts_with('-') || rest.starts_with('+') || starts_with_ordered;

            if starts_with_bullet {
                let (bullet, counter, checkbox, tag) = Self::parse_item_bullet(rest);
                items.push(ListItem {
                    position: pos,
                    indent,
                    bullet,
                    counter,
                    checkbox,
                    tag,
                });
            } else {
                break;
            }

            pos = next_pos;
        }

        Rc::new(ListStruct { items })
    }

    fn get_indent(line: &str) -> (usize, &str) {
        let mut indent = 0;
        for (i, c) in line.char_indices() {
            match c {
                ' ' => indent += 1,
                '\t' => indent += 8 - (indent % 8),
                _ => return (indent, &line[i..]),
            }
        }
        (indent, "")
    }

    fn parse_item_bullet(rest: &str) -> (&str, Option<usize>, Option<CheckBox>, Option<&str>) {
        let bytes = rest.as_bytes();
        let len = bytes.len();
        let mut offset = 0;
        let mut bullet: &str = &rest[..0];
        let mut counter = None;
        let mut checkbox = None;
        let mut tag = None;

        if offset < len {
            let c = bytes[offset];
            if c == b'-' || c == b'+' || c == b'*' {
                bullet = &rest[..1];
                offset += 1;
            } else if c.is_ascii_digit() {
                let num_start = offset;
                while offset < len && bytes[offset].is_ascii_digit() {
                    offset += 1;
                }
                if offset < len && (bytes[offset] == b'.' || bytes[offset] == b')') {
                    bullet = &rest[num_start..=offset];
                    offset += 1;
                } else {
                    offset = num_start;
                }
            }
        }

        while offset < len && (bytes[offset] == b' ' || bytes[offset] == b'\t') {
            offset += 1;
        }

        let remainder = rest[offset..].trim_start();
        let remaining = if remainder.starts_with("[@") {
            if let Some(end) = remainder.find(']') {
                let inner = &remainder[2..end];
                let num_str = inner.strip_prefix("start:").unwrap_or(inner);
                counter = num_str.parse().ok();
                remainder[end + 1..].trim_start()
            } else {
                remainder
            }
        } else {
            remainder
        };

        if remaining.starts_with('[') {
            if let Some(end) = remaining.find(']') {
                let content = &remaining[1..end];
                checkbox = match content {
                    "X" => Some(CheckBox::On),
                    " " | "" => Some(CheckBox::Off),
                    "-" => Some(CheckBox::Trans),
                    _ => None,
                };
                let after = remaining[end + 1..].trim_start();
                if let Some(tag_pos) = after.find("::") {
                    tag = Some(after[..tag_pos].trim());
                }
            }
        } else if let Some(tag_pos) = remaining.find("::") {
            tag = Some(remaining[..tag_pos].trim());
        }

        (bullet, counter, checkbox, tag)
    }
}
