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

use std::cell::RefCell;
use std::rc::Rc;

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;
use regex::Regex;

lazy_static! {
    pub static ref REGEX_TABLE_BORDER: Regex = Regex::new(r"[ \t]*\|").unwrap();
    pub static ref REGEX_TABLE_RULE: Regex = Regex::new(r"[ \t]*\+(-+\+)+[ \t]*$").unwrap();
    pub static ref REGEX_TABLE_PRE_BORDER: Regex = Regex::new(r"^[ \t]*($|[^|])").unwrap();
}

#[derive(Debug)]
pub struct TableData<'a> {
    pub tblfm: Option<&'a str>,
}

#[derive(Debug)]
pub struct TableRowData {
    pub table_row_type: TableRowType,
}

#[derive(Debug)]
pub enum TableRowType {
    Standard,
    Rule,
}

impl<'a> TableData<'a> {
    pub fn new() -> Self {
        TableData { tblfm: None }
    }
}

impl<'a> TableRowData {
    pub fn new(row_type: TableRowType) -> Self {
        TableRowData {
            table_row_type: row_type,
        }
    }
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

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::TableRow(Box::new(TableRowData::new(row_type))),
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }

    pub fn table_parser(
        &self,
        limit: usize,
        start: usize,
        _maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let mut end = start;
        let mut children: Vec<Rc<SyntaxNode<'a>>> = vec![];
        let mut current = start;

        while current < limit {
            let line_end = self.input[current..limit]
                .find('\n')
                .map_or(limit, |i| current + i + 1);

            let line = &self.input[current..line_end];

            if REGEX_TABLE_BORDER.is_match(line) || line.trim().is_empty() {
                if current > end {
                    end = current;
                }

                let row = self.parse_table_row_at(current, line_end.min(limit));
                children.push(Rc::new(row));

                current = line_end;
            } else if line.trim().is_empty() {
                current = line_end;
            } else {
                break;
            }
        }

        if children.is_empty() {
            return SyntaxNode::fallback(self.input, start, limit);
        }

        if end == start {
            end = limit;
        }

        let post_blank = if end < limit {
            let remaining = &self.input[end..limit];
            remaining.chars().take_while(|c| c.is_whitespace()).count()
        } else {
            0
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(children),
            data: Syntax::Table(Box::new(TableData::new())),
            location: Interval { start, end },
            content_location: None,
            post_blank,
            affiliated: None,
        }
    }

    fn parse_table_row_at(&self, start: usize, end: usize) -> SyntaxNode<'a> {
        let line = &self.input[start..end];

        let row_type = if REGEX_TABLE_RULE.is_match(line) {
            TableRowType::Rule
        } else {
            TableRowType::Standard
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::TableRow(Box::new(TableRowData::new(row_type))),
            location: Interval { start, end },
            content_location: None,
            post_blank: 0,
            affiliated: None,
        }
    }
}
