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
//!

use crate::affiliated::ElementSpan;
use crate::data::{BumpVec, Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use memchr::{memchr, memmem};

/// Byte-level equivalent of `REGEX_ITEM`: check if `line` (a single line
/// without trailing newline) starts with an Org list item pattern.
#[inline]
pub(crate) fn starts_with_item(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;

    // Leading whitespace
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }

    match bytes[i] {
        b'-' | b'+' => i += 1,
        b'*' => {
            // Bare '*' at the start of line is never an item.
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

    // Must be followed by whitespace or end of line
    i >= bytes.len() || bytes[i] == b' ' || bytes[i] == b'\t'
}

/// List structure - tracks items during list parsing
/// Used to compute list boundaries and parent-child relationships
#[derive(Debug)]
pub struct ListStruct<'a, 'b> {
    /// Items found so far: (position, indent, bullet, counter, checkbox, tag)
    pub items: BumpVec<'b, ListItem<'a>>,
    /// Byte position at which the list ends — the first line that is not part
    /// of the list (a headline, a less-indented line, or end of input).
    /// Set by `list_struct` when the scan terminates.
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
    /// Item's bullet (string).
    pub bullet: &'rope str,
    /// Item's check_box, if any (symbol on, off, trans, nil).
    pub checkbox: Option<CheckBox>,
    /// Item's counter, if any. Literal counters become ordinals (integer).
    pub counter: usize,
    /// Number of newline characters between the beginning
    /// of the item and the beginning of the contents (0, 1 or 2).
    pub pre_blank: usize,
    /// Uninterpreted item's tag, if any (string or nil).
    pub raw_tag: Option<&'rope str>,
    /// Parsed item's tag, if any (secondary string or nil).
    pub tag: Option<&'rope str>,
}

#[derive(Debug)]
pub struct PlainListData<'a, 'b> {
    /// Full list's structure, as returned by org_list_struct (alist).
    pub structure: &'b ListStruct<'a, 'b>,

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

/// For a description list item (`- TAG :: content`), find the byte offset
/// within `input[from..limit]` at which the description content begins —
/// right after the last ` :: ` (or ` ::\t` / ` ::` at end of line) separator.
///
/// Emacs org-element uses a greedy tag match, so the LAST ` :: ` on the
/// first line is the real separator.  Returns `None` when no separator is
/// found (item has a tag stored in the struct but no ` :: ` in the text,
/// which should not happen for well-formed org but is handled defensively).
#[inline]
fn desc_content_start(input: &str, from: usize, limit: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    // Only search on the first line of the item.
    let line_end = memchr(b'\n', &bytes[from..limit]).map_or(limit, |nl| from + nl);
    // rfind the last " :: " / " ::\t" / " ::<eol>" separator on that line.
    let line = &bytes[from..line_end];
    // Walk occurrences of "::" from right to left.
    let mut best: Option<usize> = None;
    for sep in memmem::find_iter(line, b"::") {
        // Require at least one space/tab immediately before "::".
        if sep == 0 || (line[sep - 1] != b' ' && line[sep - 1] != b'\t') {
            continue;
        }
        let after = sep + 2;
        let ok = if after >= line.len() {
            // "::" at end of line — content starts on the next line.
            true
        } else {
            line[after] == b' ' || line[after] == b'\t'
        };
        if ok {
            best = Some(sep);
        }
    }
    best.map(|sep| {
        let after = from + sep + 2; // byte just past "::"
                                    // Skip one mandatory space/tab (already verified above), plus any extras.
        let mut pos = after;
        while pos < limit && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
        // If "::" was at end of line, skip past the newline.
        if pos < limit && bytes[pos] == b'\n' {
            pos += 1;
        }
        pos
    })
}

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    /// Fallback: item parser (not yet fully implemented).
    #[inline]
    pub fn item_parser(&mut self) -> NodeId {
        let start = self.cursor.pos();
        self.arena.alloc(SyntaxNode::fallback(
            self.input,
            start,
            self.input.len(),
            self.bump,
        ))
    }

    /// Build parent relationships for a sequence of list items.
    /// Each item's parent is the nearest preceding item with strictly
    /// smaller indentation.  Returns `parent[i] = Some(p)` or `None` for
    /// top-level items.
    fn item_parents(items: &[ListItem<'a>]) -> Vec<Option<usize>> {
        let n = items.len();
        let mut parent: Vec<Option<usize>> = vec![None; n];
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for i in 0..n {
            let indent = items[i].indent;
            while let Some(&(top_i, _)) = stack.last() {
                if top_i >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }
            if let Some(&(_, p)) = stack.last() {
                parent[i] = Some(p);
            }
            stack.push((indent, i));
        }
        parent
    }

    /// Plain list parser — groups items by (parent, indent) to create
    /// separate PlainLists for each indentation level.  Items with
    /// parent=None are split by indentation so that items at different
    /// indentation levels form separate sibling PlainLists.  Items
    /// with parent=Some(p) are skipped — they are found by recursion
    /// inside their parent's content via `parse_elements`.
    /// Fixes the indent‑up‑then‑down bug where a less‑indented child
    /// got swallowed by a more‑indented preceding sibling.
    #[inline]
    pub fn plain_list_parser(
        &mut self,
        element_span: ElementSpan<'a, 'b>,
        structure: &'b ListStruct<'a, 'b>,
    ) -> NodeId {
        let span = element_span.span;
        let items = &structure.items;
        if items.is_empty() {
            return self.arena.alloc(SyntaxNode::fallback(
                self.input, span.start, span.end, self.bump,
            ));
        }

        let parent = Self::item_parents(items);

        // Build groups of consecutive items with same parent and
        // indentation.  Items with parent=Some(p) already share
        // the same indent by construction.  Items with parent=None
        // are split by indent so that each indentation level forms
        // a separate PlainList.
        let mut groups: Vec<(usize, usize)> = Vec::new();
        {
            let mut i = 0;
            while i < items.len() {
                let p = parent[i];
                let indent = items[i].indent;
                let mut j = i + 1;
                while j < items.len() {
                    if parent[j] != p {
                        break;
                    }
                    if p.is_none() && items[j].indent != indent {
                        break;
                    }
                    j += 1;
                }
                groups.push((i, j));
                i = j;
            }
        }

        let list_type = Self::get_list_type(items);
        let mut children = BumpVec::new_in(self.bump);

        use crate::parser::{ParseGranularity, ParserMode};
        let should_recurse = matches!(
            self.granularity,
            ParseGranularity::Element | ParseGranularity::Object
        );

        // Emacs' plain-list parser (org-element-plain-list-parser) advances
        // through the struct only while consecutive items share the SAME
        // indentation as the first item.  When the indent changes, the list
        // ends and a new PlainList is created by the element loop.  Mirror
        // that: only process root groups at the first root indent; a group
        // at a different indent belongs to a separate sibling PlainList.
        let first_root_indent: Option<usize> = groups
            .iter()
            .find(|&&(s, _)| parent[s].is_none())
            .map(|&(s, _)| items[s].indent);

        for &(start, end) in &groups {
            if parent[start].is_some() {
                continue; // non-top-level groups found by recursion
            }
            if first_root_indent.map_or(false, |fi| items[start].indent != fi) {
                break; // different root indent → separate PlainList, stop here
            }

            for idx in start..end {
                let item = &items[idx];
                let item_indent = item.indent;

                // End = position of the next item with indent <= this
                // item's indent (encompasses all descendants), or
                // structure.end.
                let end_pos = {
                    let mut found = None;
                    for k in (idx + 1)..items.len() {
                        if items[k].indent <= item_indent {
                            found = Some(items[k].position);
                            break;
                        }
                    }
                    found.unwrap_or(structure.end)
                };

                let item_node = self.item_parser_internal(item, end_pos);
                children.push(item_node);

                if should_recurse {
                    if let Some(loc) = self.arena[item_node].content_location {
                        let saved_ctx = self.item_indent_ctx;
                        self.item_indent_ctx = Some(item_indent);
                        let item_children = self.parse_elements(loc, ParserMode::Planning, None);
                        self.item_indent_ctx = saved_ctx;
                        self.arena.set_children(item_node, item_children);
                    }
                }
            }
        }

        let end = if let Some(last) = children.last() {
            self.arena[*last].location.end
        } else {
            span.end
        };

        let list_data = PlainListData {
            structure,
            type_s: list_type,
        };

        self.arena.alloc_with_children(
            SyntaxNode::new(Syntax::PlainList(list_data), (span.start, end), self.bump)
                .affiliated(element_span.affiliated)
                .build(),
            children,
        )
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

    fn item_content_end(&self, content_start: usize, end: usize, item_indent: usize) -> usize {
        let bytes = self.input.as_bytes();
        let mut pos = content_start;
        // Scan character-by-character looking for blank-line-separated
        // segments.  A blank line (empty or whitespace-only line) followed
        // by a line whose indent <= item_indent terminates the item content.
        while pos < end {
            let rest = &bytes[pos..];
            let nl = memchr(b'\n', rest);
            match nl {
                None => break,
                Some(nl_pos) => {
                    // Check if this line is blank (only contains whitespace up to \n)
                    let line = &rest[..nl_pos];
                    let is_blank = line.iter().all(|&b| b == b' ' || b == b'\t');
                    let next_line_pos = pos + nl_pos + 1;
                    if is_blank && next_line_pos <= end {
                        // Check the indent of the next non-blank line
                        // after the blank line.
                        let after_blank = &bytes[next_line_pos..end];
                        let nl2 = memchr(b'\n', after_blank);
                        let next_nonblank = match nl2 {
                            Some(pos2) => &after_blank[..pos2],
                            None => after_blank,
                        };
                        if !next_nonblank.is_empty() {
                            let (next_indent, _) =
                                Self::get_indent(std::str::from_utf8(next_nonblank).unwrap_or(""), self.tab_width);
                            if next_indent <= item_indent {
                                return pos;
                            }
                        } else {
                            // Blank line at the end of the range with no
                            // following non-blank content — the blank line
                            // itself is the boundary.
                            return pos;
                        }
                    }
                    pos = next_line_pos;
                }
            }
        }
        end
    }

    fn item_parser_internal(&mut self, item: &ListItem<'a>, end: usize) -> NodeId {
        let bullet = item.bullet;
        let tag = item.tag;

        // Content begins after the bullet and its trailing space/tab.
        // item.indent is screen-width (tabs expand); find the actual byte offset.
        let line_bytes = &self.input.as_bytes()[item.position..];
        let ws_end = line_bytes.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(0);
        let after_bullet = item.position + ws_end + item.bullet.len() + 1;

        // For description items the paragraph content starts after the " :: "
        // separator, not at the tag.  Emacs org-element places :contents-begin
        // there, so the paragraph :begin must match.
        let content_start = if tag.is_some() {
            desc_content_start(self.input, after_bullet, end).unwrap_or(after_bullet)
        } else if item.checkbox.is_some() {
            after_bullet + 4 // skip "[ ] " (checkbox + trailing space)
        } else {
            after_bullet
        };

        // Cap content end at the first blank-line-separated line whose
        // indent is <= this item's indent.  This ensures blank lines
        // before lower-indent elements close the item.
        let effective_content_end = self.item_content_end(content_start, end, item.indent);

        let content_location = (content_start < effective_content_end).then_some(Interval {
            start: content_start,
            end: effective_content_end,
        });

        let item_data = ItemData {
            bullet,
            checkbox: item.checkbox.clone(),
            counter: item.counter.unwrap_or(0),
            pre_blank: 0,
            raw_tag: tag,
            tag,
        };

        let mut node = SyntaxNode::new(
            Syntax::Item(self.bump.alloc(item_data)),
            (item.position, end),
            self.bump,
        );
        if let Some(loc) = content_location {
            node = node.content(loc);
        }
        self.arena.alloc(node.build())
    }

    /// Scan input for list items at current indentation level
    /// Matches Elisp's org-element--list-struct
    #[inline]
    pub fn list_struct(&self, limit: usize) -> &'b ListStruct<'a, 'b> {
        let bump = self.bump;
        let mut items = BumpVec::new_in(bump);
        let mut pos = self.cursor.pos();
        let input = self.input;

        // The indent of the first item anchors what counts as a continuation
        // line (any non-bullet, non-blank line indented MORE than this stops
        // the scan only if it is at the same-or-lower indent level).
        let mut first_indent: Option<usize> = None;

        // Track whether we are inside a #+BEGIN_ / #+END_ block so that
        // body lines at any indent are not mistaken for list terminators.
        let mut in_block: bool = false;

        while pos < limit {
            let (line, next_pos) = match memchr(b'\n', &input.as_bytes()[pos..]) {
                Some(nl_pos) => (&input[pos..pos + nl_pos], pos + nl_pos + 1),
                None => (&input[pos..], limit),
            };

            let (indent, rest) = Self::get_indent(line, self.tab_width);

            // Block-aware skipping: lines between #+BEGIN_ and #+END_
            // at indent > first_indent are body lines inside a list
            // item and must not terminate the list.  Blocks at the
            // same-or-lower indent as the list itself are top-level
            // elements and DO terminate the list.
            let rest_bytes = rest.as_bytes();
            if !in_block
                && first_indent.is_some()
                && indent > first_indent.unwrap()
                && rest_bytes.len() >= 7
                && rest_bytes[0] == b'#'
                && rest_bytes[1] == b'+'
                && (rest_bytes[2..7]).eq_ignore_ascii_case(b"BEGIN")
            {
                in_block = true;
                pos = next_pos;
                continue;
            }

            if in_block {
                if rest_bytes.len() >= 5
                    && rest_bytes[0] == b'#'
                    && rest_bytes[1] == b'+'
                    && (rest_bytes[2..5]).eq_ignore_ascii_case(b"END")
                {
                    in_block = false;
                }
                pos = next_pos;
                continue;
            }

            if rest.is_empty() {
                // Two or more consecutive blank lines terminate the list
                // (and the current item) per the Org syntax spec.
                let blank_rest = &input.as_bytes()[next_pos..];
                let second_nl = memchr(b'\n', blank_rest);
                if let Some(nl2) = second_nl {
                    let second_line = &blank_rest[..nl2];
                    if second_line.iter().all(|&b| b == b' ' || b == b'\t') {
                        // Second blank line found — terminate the list at
                        // the first blank line (current pos before consuming it).
                        break;
                    }
                }
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
            let starts_with_bullet = {
                let bs = rest.as_bytes();
                let first = bs.first().copied();
                (first == Some(b'-') || first == Some(b'+') || first == Some(b'*'))
                    && (bs.len() == 1 || bs[1] == b' ' || bs[1] == b'\t')
                    || starts_with_ordered
            };

            if starts_with_bullet {
                if let Some(fi) = first_indent {
                    match self.item_indent_ctx {
                        None => {
                            // Top-level scan: a bullet at lower indent than the
                            // first item starts a new list at an outer scope.
                            if indent < fi {
                                break;
                            }
                        }
                        Some(parent_indent) => {
                            // Inside an item: only stop when the bullet exits the
                            // parent item's scope entirely (indent <= parent's indent).
                            if indent <= parent_indent {
                                break;
                            }
                        }
                    }
                }
                first_indent.get_or_insert(indent);
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
                // Non-bullet, non-blank line.  It is a continuation of the
                // current item if it is indented strictly MORE than the first
                // item's bullet.  Otherwise it terminates the list.
                let fi = first_indent.unwrap_or(0);
                if indent > fi {
                    pos = next_pos;
                    continue;
                }
                break;
            }

            pos = next_pos;
        }

        bump.alloc(ListStruct { items, end: pos })
    }

    fn get_indent(line: &str, tab_width: u8) -> (usize, &str) {
        let mut indent = 0;
        for (i, c) in line.char_indices() {
            match c {
                ' ' => indent += 1,
                '\t' => indent += tab_width as usize,
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
