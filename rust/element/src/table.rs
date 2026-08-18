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

use crate::data::{BumpVec, NodeId};
use crate::prelude::*;
use memchr::memchr;

#[derive(Debug)]
#[repr(u8)]
pub enum TableRowType {
    Standard,
    Rule,
}

/// `^[ \t]*\|` — line is a table border (pipe-delimited row or rule).
#[inline]
fn is_table_line(bytes: &[u8]) -> bool {
    let i = bytes
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    bytes.get(i) == Some(&b'|')
}

/// `^[ \t]*\|-+` — line is a table horizontal rule.
#[inline]
fn is_table_rule(bytes: &[u8]) -> bool {
    let i = bytes
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    bytes.get(i) == Some(&b'|') && bytes.get(i + 1) == Some(&b'-')
}

/// `(?i)^[ \t]*#\+TBLFM:` — line is a table formula annotation.
#[inline]
fn is_tblfm(bytes: &[u8]) -> bool {
    let i = bytes
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    bytes
        .get(i..i + 8)
        .is_some_and(|s| s.eq_ignore_ascii_case(b"#+TBLFM:"))
}

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    #[inline]
    pub fn table_row_parser(&mut self) -> NodeId {
        let start = self.cursor.pos();
        let limit = self.input.len();

        let end = memchr(b'\n', &self.input.as_bytes()[start..limit])
            .map_or(limit, |i| (start + i + 1).min(limit));

        let row_type = if is_table_rule(&self.input.as_bytes()[start..end]) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        self.arena
            .alloc(SyntaxNode::new(Syntax::TableRow(row_type), (start, end), self.bump).build())
    }

    #[inline]
    pub fn table_parser(&mut self, element_span: ElementSpan<'a, 'b>) -> NodeId {
        let span = element_span.span;
        let (end, children) = {
            let mut current = self.cursor.pos().max(span.start);
            let mut rows: BumpVec<'b, NodeId> = BumpVec::new_in(self.bump);
            loop {
                if current >= span.end {
                    break;
                }
                let line_end = memchr(b'\n', &self.input.as_bytes()[current..span.end])
                    .map_or(span.end, |i| current + i + 1);
                let line = &self.input[current..line_end];
                if line.trim().is_empty() || !is_table_line(line.as_bytes()) {
                    break;
                }
                rows.push(self.parse_table_row_at(current, line_end));
                current = line_end;
            }
            (current, rows)
        };

        if children.is_empty() {
            return self.arena.alloc(SyntaxNode::fallback(
                self.input, span.start, span.end, self.bump,
            ));
        }

        let formula_end = self.input[end..span.end]
            .lines()
            .next()
            .and_then(|first_line| {
                if is_tblfm(first_line.as_bytes()) {
                    let line_end = end + first_line.len();
                    Some(
                        if line_end < span.end && self.input.as_bytes()[line_end] == b'\n' {
                            line_end + 1
                        } else {
                            line_end
                        },
                    )
                } else {
                    None
                }
            });

        let actual_end = formula_end.unwrap_or(end);

        let post_blank = if actual_end < span.end {
            let remaining = &self.input[actual_end..span.end];
            remaining.len() - remaining.trim_start().len()
        } else {
            0
        };

        let node = SyntaxNode::new(Syntax::Table, (span.start, actual_end), self.bump)
            .post_blank(post_blank)
            .affiliated(element_span.affiliated)
            .build();
        self.arena.alloc_with_children(node, children)
    }

    fn parse_table_row_at(&mut self, start: usize, end: usize) -> NodeId {
        let line = &self.input[start..end];

        let row_type = if is_table_rule(line.as_bytes()) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        #[allow(
            clippy::obfuscated_if_else,
            reason = "This is not obfuscated, just my style"
        )]
        let cells = (matches!(row_type, TableRowType::Standard)
            && self.granularity == ParseGranularity::Object)
            .then(|| self.parse_table_cells(start, end))
            .unwrap_or_else(|| BumpVec::new_in(self.bump));

        let node = SyntaxNode::new(Syntax::TableRow(row_type), (start, end), self.bump).build();
        self.arena.alloc_with_children(node, cells)
    }

    /// Split a standard table row into `TableCell` nodes.
    ///
    /// Emacs wraps each `|…|` segment in a `table-cell` element whose extent
    /// runs from the character immediately after the preceding `|` up to and
    /// including the following `|`.  Object content within the cell is trimmed
    /// of leading/trailing whitespace before parsing.
    fn parse_table_cells(&mut self, row_start: usize, row_end: usize) -> BumpVec<'b, NodeId> {
        #[allow(
            clippy::sliced_string_as_bytes,
            reason = "We actually want to slice by the UTF-8 boundaries first."
        )]
        let bytes = self.input[row_start..row_end].as_bytes();
        let mut cells = BumpVec::new_in(self.bump);

        let first_pipe = match memchr(b'|', bytes) {
            Some(i) => i,
            None => return cells,
        };
        let mut cell_begin = row_start + first_pipe + 1;

        while cell_begin < row_end {
            let local = cell_begin - row_start;
            let pipe_rel = match memchr(b'|', &bytes[local..]) {
                Some(i) => i,
                None => break,
            };
            let pipe_abs = cell_begin + pipe_rel;
            let cell_end = pipe_abs + 1;

            let raw = &self.input[cell_begin..pipe_abs];
            let leading = raw.bytes().take_while(|&b| b == b' ' || b == b'\t').count();
            let trailing = raw
                .bytes()
                .rev()
                .take_while(|&b| b == b' ' || b == b'\t')
                .count();
            let contents_begin = cell_begin + leading;
            let contents_end = pipe_abs.saturating_sub(trailing).max(contents_begin);

            let cell_node = self.arena.alloc(
                SyntaxNode::new(Syntax::TableCell, (cell_begin, cell_end), self.bump)
                    .content((contents_begin, contents_end))
                    .build(),
            );

            let children = self.parse_objects((contents_begin, contents_end), |that| {
                SyntaxT::TableCell.can_contain(that)
            });
            self.arena.set_children(cell_node, children);
            cells.push(cell_node);

            cell_begin = cell_end;
        }

        cells
    }
}
