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

use crate::data::NodeId;
use crate::prelude::*;
use memchr::memchr;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_TABLE_BORDER: Regex = Regex::new(r"[ \t]*\|").unwrap();
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
    fn from(n: usize) -> Self { Row(n) }
}

impl From<usize> for Col {
    #[inline]
    fn from(n: usize) -> Self { Col(n) }
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

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    #[inline]
    pub fn table_row_parser(&mut self) -> NodeId {
        let start = self.cursor.pos();
        let limit = self.input.len();

        let end = memchr(b'\n', self.input[start..limit].as_bytes())
            .map_or(limit, |i| (start + i + 1).min(limit));

        let row_type = if REGEX_TABLE_RULE.is_match(&self.input[start..end]) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        self.arena.alloc(SyntaxNode::new(Syntax::TableRow(row_type), (start, end)).build())
    }

    #[inline]
    pub fn table_parser(&mut self, element_span: ElementSpan<'a>) -> NodeId {
        let span = element_span.span;
        let (end, children) = {
            let mut current = span.start;
            let mut rows: Vec<NodeId> = Vec::new();
            loop {
                if current >= span.end { break; }
                let line_end = memchr(b'\n', self.input[current..span.end].as_bytes())
                    .map_or(span.end, |i| current + i + 1);
                let line = &self.input[current..line_end];
                if line.trim().is_empty() || !REGEX_TABLE_BORDER.is_match(line) { break; }
                rows.push(self.parse_table_row_at(current, line_end));
                current = line_end;
            }
            (current, rows)
        };

        if children.is_empty() {
            return self.arena.alloc(SyntaxNode::fallback(self.input, span.start, span.end));
        }

        // Look for a #+TBLFM: line immediately after the table rows.
        let formula = self.input[end..span.end]
            .lines()
            .find_map(|line| REGEX_TBLFM.captures(line).map(|c| c.get(1).map_or("", |m| m.as_str().trim())));

        let post_blank = (end < span.end).then(|| {
            let remaining = &self.input[end..span.end];
            remaining.len() - remaining.trim_start().len()
        })
        .unwrap_or(0);

        match formula {
            None => {
                let node = SyntaxNode::new(Syntax::Table, (span.start, end))
                    .post_blank(post_blank)
                    .affiliated(element_span.affiliated)
                    .build();
                self.arena.alloc_with_children(node, children)
            }
            Some(formula) => {
                let row_count = Row(children.iter()
                    .filter(|&&r| matches!(&self.arena[r].data, Syntax::TableRow(TableRowType::Standard)))
                    .count());
                let col_count = Col(children.first()
                    .map(|&r| self.arena[r].children.len())
                    .unwrap_or(0));
                let node = SyntaxNode::new(
                    Syntax::Spreadsheet(Box::new(SpreadsheetData { formula, row_count, col_count })),
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

        let content_location = (matches!(row_type, TableRowType::Standard)
            && self.granularity == ParseGranularity::Object)
            .then(|| (start, (end - 1).max(start)));

        let children = content_location
            .map(|loc| self.parse_objects(loc, |that| SyntaxT::TableCell.can_contain(that)))
            .unwrap_or_default();

        let mut builder = SyntaxNode::new(Syntax::TableRow(row_type), (start, end));
        if let Some(loc) = content_location {
            builder = builder.content(loc);
        }
        let node = builder.build();
        self.arena.alloc_with_children(node, children)
    }
}
