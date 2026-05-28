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

use std::fmt;

use crate::cursor::CachedRegex;
use crate::data::NodeId;
use crate::prelude::*;
use memchr::memchr;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_TABLE_BORDER: CachedRegex =
        CachedRegex::new(Regex::new(r"[ \t]*\|").unwrap());
    pub static ref REGEX_TABLE_RULE: Regex = Regex::new(r"^[ \t]*\|-+").unwrap();
    pub static ref REGEX_TABLE_PRE_BORDER: Regex = Regex::new(r"^[ \t]*($|[^|])").unwrap();
    static ref REGEX_TBLFM: Regex = Regex::new(r"(?i)^[ \t]*#\+TBLFM:(.*)").unwrap();
}

/// A 1-based row index in a spreadsheet.  Displays as `@N` (org Calc syntax).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Row(pub usize);

/// A 1-based column index in a spreadsheet.  Displays as `$N` (org Calc syntax).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Col(pub usize);

impl fmt::Display for Row {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl fmt::Display for Col {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl From<usize> for Row {
    #[inline]
    fn from(n: usize) -> Self {
        Row(n)
    }
}

impl From<usize> for Col {
    #[inline]
    fn from(n: usize) -> Self {
        Col(n)
    }
}

/// Data for a `#+TBLFM` spreadsheet table.
#[derive(Debug)]
pub struct SpreadsheetData<'a> {
    /// The raw `#+TBLFM:` formula string.
    pub formula: &'a str,
    pub row_count: Row,
    pub col_count: Col,
}

/// A row inside a [`Syntax::Spreadsheet`].
#[derive(Debug)]
pub struct SpreadsheetRowData {
    /// 1-based data-row index (rule rows do not increment this).
    pub index: Row,
    /// `true` for `|---|` separator rows.
    pub is_rule: bool,
}

/// A single cell inside a [`Syntax::SpreadsheetRow`].
#[derive(Debug)]
pub struct SpreadsheetCellData<'a> {
    /// Raw cell content (not yet formula-evaluated).
    pub value: &'a str,
    pub row: Row,
    pub col: Col,
}

#[derive(Debug)]
#[repr(u8)]
pub enum TableRowType {
    Standard,
    Rule,
}

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    #[inline]
    pub fn table_row_parser(&mut self) -> NodeId {
        let start = self.cursor.pos();
        let limit = self.input.len();

        let end = memchr(b'\n', &self.input.as_bytes()[start..limit])
            .map_or(limit, |i| (start + i + 1).min(limit));

        let row_type = if REGEX_TABLE_RULE.is_match(&self.input[start..end]) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        self.arena
            .alloc(SyntaxNode::new(Syntax::TableRow(row_type), (start, end)).build())
    }

    #[inline]
    pub fn table_parser(&mut self, element_span: ElementSpan<'a, 'b>) -> NodeId {
        let span = element_span.span;
        let (end, children) = {
            let mut current = span.start;
            let mut rows: Vec<NodeId> = Vec::new();
            loop {
                if current >= span.end {
                    break;
                }
                let line_end = memchr(b'\n', &self.input.as_bytes()[current..span.end])
                    .map_or(span.end, |i| current + i + 1);
                let line = &self.input[current..line_end];
                if line.trim().is_empty() || !REGEX_TABLE_BORDER.is_match(line) {
                    break;
                }
                rows.push(self.parse_table_row_at(current, line_end));
                current = line_end;
            }
            (current, rows)
        };

        if children.is_empty() {
            return self
                .arena
                .alloc(SyntaxNode::fallback(self.input, span.start, span.end));
        }

        // Look for a #+TBLFM: line immediately after the table rows.
        let formula = self.input[end..span.end].lines().find_map(|line| {
            REGEX_TBLFM
                .captures(line)
                .map(|c| c.get(1).map_or("", |m| m.as_str().trim()))
        });

        let post_blank = if end < span.end {
            let remaining = &self.input[end..span.end];
            remaining.len() - remaining.trim_start().len()
        } else {
            0
        };

        match formula {
            None => {
                let node = SyntaxNode::new(Syntax::Table, (span.start, end))
                    .post_blank(post_blank)
                    .affiliated(element_span.affiliated)
                    .build();
                self.arena.alloc_with_children(node, children)
            }
            Some(formula) => {
                let row_count = Row(children
                    .iter()
                    .filter(|&&r| {
                        matches!(
                            &self.arena[r].data,
                            Syntax::TableRow(TableRowType::Standard)
                        )
                    })
                    .count());
                let col_count = Col(children
                    .first()
                    .map(|&r| self.arena[r].children.len())
                    .unwrap_or(0));
                let node = SyntaxNode::new(
                    Syntax::Spreadsheet(self.bump.alloc(SpreadsheetData {
                        formula,
                        row_count,
                        col_count,
                    })),
                    (span.start, end),
                )
                .post_blank(post_blank)
                .affiliated(element_span.affiliated)
                .build();
                self.arena.alloc_with_children(node, children)
            }
        }
    }

    fn parse_table_row_at(&mut self, start: usize, end: usize) -> NodeId {
        let line = &self.input[start..end];

        let row_type = if REGEX_TABLE_RULE.is_match(line) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        let cells = (matches!(row_type, TableRowType::Standard)
            && self.granularity == ParseGranularity::Object)
            .then(|| self.parse_table_cells(start, end))
            .unwrap_or_default();

        let node = SyntaxNode::new(Syntax::TableRow(row_type), (start, end)).build();
        self.arena.alloc_with_children(node, cells)
    }

    /// Split a standard table row into `TableCell` nodes.
    ///
    /// Emacs wraps each `|…|` segment in a `table-cell` element whose extent
    /// runs from the character immediately after the preceding `|` up to and
    /// including the following `|`.  Object content within the cell is trimmed
    /// of leading/trailing whitespace before parsing.
    fn parse_table_cells(&mut self, row_start: usize, row_end: usize) -> Vec<NodeId> {
        let bytes = self.input[row_start..row_end].as_bytes();
        let mut cells = Vec::new();

        // Skip leading whitespace then the opening '|'.
        let first_pipe = match memchr(b'|', bytes) {
            Some(i) => i,
            None => return cells,
        };
        let mut cell_begin = row_start + first_pipe + 1;

        while cell_begin < row_end {
            // Find the '|' that closes this cell.
            let local = cell_begin - row_start;
            let pipe_rel = match memchr(b'|', &bytes[local..]) {
                Some(i) => i,
                None => break,
            };
            let pipe_abs = cell_begin + pipe_rel;
            let cell_end = pipe_abs + 1; // inclusive of the closing '|'

            // Trim whitespace to get content boundaries for parse_objects.
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
                SyntaxNode::new(Syntax::TableCell, (cell_begin, cell_end))
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
