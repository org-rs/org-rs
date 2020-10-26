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
use crate::data::SyntaxNode;
use crate::parser::Parser;
use regex::Regex;
use std::borrow::Cow;
use std::cell::Cell;
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

/// List structure
/// This looks like an intermediate list representation, required both by
/// plain list itself and items in the list.
#[derive(Debug)]
pub struct ListStruct {
    // stub
}

#[derive(Debug)]
pub struct ItemData<'rope> {
    /// Item's bullet (string).
    bullet: Cow<'rope, str>,
    /// Item's check_box, if any (symbol on, off, trans, nil).
    checkbox: Option<CheckBox>,
    /// Item's counter, if any. Literal counters become ordinals (integer).
    counter: usize,
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

#[derive(Debug)]
pub enum CheckBox {
    On,
    Off,
    Trans,
}

impl<'a> Parser<'a> {
    // TODO implement item_parser
    //https://code.orgmode.org/bzg/org-mode/src/master/lisp/org-element.el#L1253
    pub fn item_parser(
        &self,
        structure: Option<Rc<ListStruct>>,
        raw_secondary_p: bool,
    ) -> SyntaxNode<'a> {
        //   let mut item_data = ItemData {
        //       bullet: (),
        //       checkbox: None,
        //       counter: 0,
        //       pre_blank: 0,
        //       raw_tag: None,
        //       tag: None,
        //       structure: ListStruct {}
        //   }

        //    let mut node = SyntaxNode {
        //        parent: Cell::new(None),
        //        children: (),
        //        data: item_data
        //        location: Interval {},
        //        content_location: None,
        //        post_blank: 0,
        //        affiliated: None
        //    }
        unimplemented!()
    }

    // TODO implement plain_list_parser
    pub fn plain_list_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData>,
        structure: Rc<ListStruct>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }

    //(defun org-element--list-struct (limit)
    ///  ;; Return structure of list at point.  Internal function.  See
    ///  ;; `org-list-struct' for details.
    pub fn list_struct(&self, limit: usize) -> Rc<ListStruct> {
        unimplemented!()
    }
}
