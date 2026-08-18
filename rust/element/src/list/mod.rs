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
//!  Items are defined by a line starting with the following pattern: "BULLET
//! COUNTER-SET CHECK-BOX TAG", in which only BULLET is mandatory.
//!
//!  BULLET is either an asterisk, a hyphen, a plus sign character or follows
//! either the pattern "COUNTER." or "COUNTER)".  In any case, BULLET is follwed by
//! a whitespace character or line ending.
//!
//!  COUNTER can be a number or a single letter.
//!
//!  COUNTER-SET follows the pattern [@COUNTER].
//!
//!  CHECK-BOX is either a single whitespace character, a "X" character or a
//! hyphen, enclosed within square brackets.
//!
//!  TAG follows "TAG-TEXT ::" pattern, where TAG-TEXT can contain any character
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
//! be an "ordered plain-list". If it contains a tag, it will be a "descriptive
//! list". Otherwise, it will be an "unordered list". List types are mutually
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

mod parser;

use crate::data::BumpVec;
use memchr::{memchr, memmem};

/// Byte-level equivalent of `REGEX_ITEM`: check if `line` (a single line
/// without trailing newline) starts with an Org list item pattern.
#[inline]
pub(crate) fn starts_with_item(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }

    match bytes[i] {
        b'-' | b'+' => i += 1,
        b'*' => {
            if i == 0 {
                return false;
            }
            i += 1;
        }
        _ if bytes[i].is_ascii_digit() => {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i >= bytes.len() || (bytes[i] != b'.' && bytes[i] != b')') {
                return false;
            }
            i += 1;
        }
        _ => return false,
    }

    i >= bytes.len() || bytes[i] == b' ' || bytes[i] == b'\t'
}

#[derive(Debug)]
pub struct ListStruct<'a, 'b> {
    pub items: BumpVec<'b, ListItem<'a>>,
    pub end: usize,
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

impl<'a, 'b> ListStruct<'a, 'b> {
    #[inline]
    pub fn new_in(bump: &'b bumpalo::Bump) -> Self {
        ListStruct {
            items: BumpVec::new_in(bump),
            end: 0,
        }
    }
}

#[derive(Debug)]
pub struct ItemData<'rope> {
    pub bullet: &'rope str,
    pub checkbox: Option<CheckBox>,
    pub counter: usize,
    pub pre_blank: usize,
    pub raw_tag: Option<&'rope str>,
    pub tag: Option<&'rope str>,
}

#[derive(Debug)]
pub struct PlainListData<'a, 'b> {
    pub structure: &'b ListStruct<'a, 'b>,
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

/// For a description list item (`- TAG :: content`), find the byte offset
/// within `input[from..limit]` at which the description content begins —
/// right after the last ` :: ` (or ` ::\t` / ` ::` at end of line) separator.
pub(super) fn desc_content_start(input: &str, from: usize, limit: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let line_end = memchr(b'\n', &bytes[from..limit]).map_or(limit, |nl| from + nl);
    let line = &bytes[from..line_end];
    let mut best: Option<usize> = None;
    for sep in memmem::find_iter(line, b"::") {
        if sep == 0 || (line[sep - 1] != b' ' && line[sep - 1] != b'\t') {
            continue;
        }
        let after = sep + 2;
        let ok = if after >= line.len() {
            true
        } else {
            line[after] == b' ' || line[after] == b'\t'
        };
        if ok {
            best = Some(sep);
        }
    }
    best.map(|sep| {
        let after = from + sep + 2;
        let mut pos = after;
        while pos < limit && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
        if pos < limit && bytes[pos] == b'\n' {
            pos += 1;
        }
        pos
    })
}
