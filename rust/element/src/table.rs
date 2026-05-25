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
use std::rc::Rc;

use crate::prelude::*;
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl fmt::Display for Col {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl From<usize> for Row {
    fn from(n: usize) -> Self { Row(n) }
}

impl From<usize> for Col {
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
pub enum TableRowType {
    Standard,
    Rule,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    pub fn table_row_parser(&self) -> SyntaxNode<'a> {
        let start = self.cursor.borrow().pos();
        let limit = self.input.len();

        let end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| (start + i + 1).min(limit));

        let row_type = if REGEX_TABLE_RULE.is_match(&self.input[start..end]) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        SyntaxNode::new(Syntax::TableRow(row_type), (start, end)).build()
    }

    pub fn table_parser(&self, element_span: ElementSpan<'a>) -> SyntaxNode<'a> {
        let span = element_span.span;
        let (end, children) = {
            let mut current = span.start;
            let mut rows: Vec<Rc<SyntaxNode<'a>>> = Vec::new();
            loop {
                if current >= span.end { break; }
                let line_end = self.input[current..span.end]
                    .find('\n')
                    .map_or(span.end, |i| current + i + 1);
                let line = &self.input[current..line_end];
                if line.trim().is_empty() || !REGEX_TABLE_BORDER.is_match(line) { break; }
                rows.push(Rc::new(self.parse_table_row_at(current, line_end)));
                current = line_end;
            }
            (current, rows)
        };

        if children.is_empty() {
            return SyntaxNode::fallback(self.input, span.start, span.end);
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
            None => SyntaxNode::new(Syntax::Table, (span.start, end))
                .post_blank(post_blank)
                .affiliated(element_span.affiliated)
                .children(children)
                .build(),
            Some(formula) => {
                let row_count = Row(children.iter()
                    .filter(|r| matches!(&r.data, Syntax::TableRow(TableRowType::Standard)))
                    .count());
                let col_count = Col(children.first()
                    .map(|r| r.children.borrow().len())
                    .unwrap_or(0));
                SyntaxNode::new(
                    Syntax::Spreadsheet(Box::new(SpreadsheetData { formula, row_count, col_count })),
                    (span.start, end),
                )
                .post_blank(post_blank)
                .affiliated(element_span.affiliated)
                .children(children)
                .build()
            }
        }
    }

    fn parse_table_row_at(&self, start: usize, end: usize) -> SyntaxNode<'a> {
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

        let mut builder = SyntaxNode::new(Syntax::TableRow(row_type), (start, end))
            .children(children);
        if let Some(loc) = content_location {
            builder = builder.content(loc);
        }
        builder.build()
    }
}
