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

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;
use regex::Regex;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

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
pub struct ListStruct {
    /// Items found so far: (position, indent, bullet, counter, checkbox, tag)
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub position: usize,
    pub indent: usize,
    pub bullet: String,
    pub counter: Option<usize>,
    pub checkbox: Option<CheckBox>,
    pub tag: Option<String>,
}

impl ListStruct {
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
    structure: ListStruct,
}

#[derive(Debug)]
pub struct PlainListData {
    /// Full list's structure, as returned by org_list_struct (alist).
    pub structure: Rc<ListStruct>,

    ///List's type (symbol descriptive, ordered, unordered).
    pub type_s: ListKind,
}

#[derive(Debug)]
pub enum ListKind {
    Descriptive,
    Ordered,
    Unordered,
}

#[derive(Debug, Clone)]
pub enum CheckBox {
    On,
    Off,
    Trans,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Fallback: item parser (not yet fully implemented).
    pub fn item_parser(
        &self,
        _structure: Option<Rc<ListStruct>>,
        _raw_secondary_p: bool,
    ) -> SyntaxNode<'a> {
        let start = self.cursor.borrow().pos();
        SyntaxNode::fallback(self.input, start, self.input.len())
    }

    /// Fallback: plain list parser (not yet fully implemented).
    pub fn plain_list_parser(
        &self,
        limit: usize,
        start: usize,
        _affiliated: Option<AffiliatedData>,
        structure: Rc<ListStruct>,
    ) -> SyntaxNode<'a> {
        let items = &structure.items;
        if items.is_empty() {
            return SyntaxNode::fallback(self.input, start, limit);
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
                limit
            };

            let item_node = self.item_parser_internal(item, end_pos);
            children.push(Rc::new(item_node));
            i += 1;
        }

        let end = if let Some(last) = children.last() {
            last.location.end
        } else {
            limit
        };

        let list_data = PlainListData {
            structure: structure.clone(),
            type_s: list_type,
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(children),
            data: Syntax::PlainList(Box::new(list_data)),
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    fn get_list_type(items: &[ListItem]) -> ListKind {
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

    fn item_parser_internal(&self, item: &ListItem, end: usize) -> SyntaxNode<'a> {
        let bullet = Cow::Owned(item.bullet.clone());
        let tag = item.tag.as_ref().map(|t| Cow::Owned(t.clone()));

        let item_data = ItemData {
            bullet,
            checkbox: item.checkbox.clone(),
            counter: item.counter.unwrap_or(0),
            pre_blank: 0,
            raw_tag: tag.clone(),
            tag,
            structure: ListStruct::new(),
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Item(Box::new(item_data)),
            location: Interval {
                start: item.position,
                end,
            },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    /// Scan input for list items at current indentation level
    /// This matches Elisp's org-element--list-struct
    pub fn list_struct(&self, limit: usize) -> Rc<ListStruct> {
        let mut items = Vec::new();
        let mut pos = self.cursor.borrow().pos();
        let input = self.input;

        while pos < limit {
            let (line, next_pos) = match input[pos..].find('\n') {
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
                while j < bs.len() && bs[j].is_ascii_digit() { j += 1; }
                j > 0 && j < bs.len()
                    && (bs[j] == b'.' || bs[j] == b')')
                    && (j + 1 >= bs.len() || bs[j + 1] == b' ' || bs[j + 1] == b'\t')
            };
            let starts_with_bullet = rest.starts_with('-')
                || rest.starts_with('+')
                || starts_with_ordered;

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

    fn parse_item_bullet(rest: &str) -> (String, Option<usize>, Option<CheckBox>, Option<String>) {
        let mut chars = rest.chars().peekable();
        let mut bullet = String::new();
        let mut counter = None;
        let mut checkbox = None;
        let mut tag = None;

        if let Some(&c) = chars.peek() {
            if c == '-' || c == '+' || c == '*' {
                bullet.push(chars.next().unwrap());
            } else if c.is_ascii_digit() {
                let mut num = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if let Some(&c2) = chars.peek() {
                    if c2 == '.' || c2 == ')' {
                        bullet = num.clone();
                        bullet.push(chars.next().unwrap());
                    }
                }
            }
        }

        while let Some(c) = chars.next() {
            if c == ' ' || c == '\t' {
                break;
            }
        }

        let remaining: String = chars.collect();
        let rest = if remaining.starts_with("[@") {
            // Counter set: [@N] or [@start:N]
            if let Some(end) = remaining.find(']') {
                let inner = &remaining[2..end];
                let num_str = inner.strip_prefix("start:").unwrap_or(inner);
                counter = num_str.parse().ok();
                remaining[end + 1..].trim_start().to_string()
            } else {
                remaining.clone()
            }
        } else {
            remaining.clone()
        };

        if rest.starts_with('[') {
            if let Some(end) = rest.find(']') {
                let content = &rest[1..end];
                checkbox = match content {
                    "X" => Some(CheckBox::On),
                    " " | "" => Some(CheckBox::Off),
                    "-" => Some(CheckBox::Trans),
                    _ => None,
                };
                let after = rest[end + 1..].trim_start();
                if let Some(tag_pos) = after.find("::") {
                    tag = Some(after[tag_pos + 2..].trim().to_string());
                }
            }
        } else if let Some(tag_pos) = rest.find("::") {
            tag = Some(rest[tag_pos + 2..].trim().to_string());
        }

        (bullet, counter, checkbox, tag)
    }
}
