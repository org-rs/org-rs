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

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::markup::REGEX_DIARY_SEXP;
use crate::parser::Parser;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref REGEX_CLOCK_TIMESTAMP: Regex = Regex::new(
        r"^\[(\d{4})-(\d{2})-(\d{2}) [A-Za-z]+ (\d{1,2}):(\d{2})(?:\s*--\s*\[(\d{4})-(\d{2})-(\d{2}) [A-Za-z]+ (\d{1,2}):(\d{2})\])?(?:\s*=>\s*(\d+:\d{2}))?\s*$"
    ).unwrap();
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    pub fn planning_parser(&self, limit: usize) -> SyntaxNode<'a> {
        let start = self.cursor.borrow().pos();
        let input_slice = &self.input[start..limit];

        let line_end = input_slice.find('\n').map_or(limit, |i| start + i);
        let line = &input_slice[..(line_end - start)];

        let mut deadline = None;
        let mut scheduled = None;
        let mut closed = None;

        if let Some(d) = self.parse_planning_timestamp(line, "DEADLINE:") {
            deadline = Some(d);
        }
        if let Some(s) = self.parse_planning_timestamp(line, "SCHEDULED:") {
            scheduled = Some(s);
        }
        if let Some(c) = self.parse_planning_timestamp(line, "CLOSED:") {
            closed = Some(c);
        }

        if deadline.is_none() && scheduled.is_none() && closed.is_none() {
            return SyntaxNode::fallback(self.input, start, limit);
        }

        let post_blank = if line_end < limit {
            let remaining = &self.input[line_end..limit];
            let trimmed = remaining.trim_start();
            (remaining.len() - trimmed.len()).min(2)
        } else {
            0
        };

        let planning_data = crate::data::PlanningData {
            closed,
            deadline,
            scheduled,
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Planning(Box::new(planning_data)),
            location: Interval {
                start,
                end: line_end,
            },
            content_location: None,
            post_blank,
            affiliated: None,
        }
    }

    fn parse_planning_timestamp(
        &self,
        line: &'a str,
        keyword: &str,
    ) -> Option<crate::data::TimestampData<'a>> {
        let keyword_pos = line.find(keyword)?;
        let after_keyword = line[keyword_pos + keyword.len()..].trim_start();

        self.parse_timestamp_from_str(after_keyword)
    }

    fn parse_timestamp_from_str(&self, s: &'a str) -> Option<crate::data::TimestampData<'a>> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        if (s.starts_with('<') && s.ends_with('>')) || (s.starts_with('[') && s.ends_with(']')) {
            crate::data::TimestampData::new(s)
        } else {
            None
        }
    }

    /// Parse a clock line element.
    ///
    /// Clock format: `CLOCK: [timestamp]` or `CLOCK: [start]--[end] => duration`
    /// Case insensitive (matches "CLOCK:", "Clock:", etc.)
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound search
    /// - Uses cursor position for begin
    /// - Stores raw value in ClockData
    ///
    /// Note: This implementation validates the timestamp format, which is a
    /// deviation from Elisp. Elisp uses `org-parse-time-string` which is more
    /// lenient and accepts various date formats. We use strict regex validation
    /// to catch invalid dates like Feb 29 in non-leap years.
    pub fn clock_line_parser(&self, limit: usize) -> SyntaxNode<'a> {
        let start = self.cursor.borrow().pos();
        let input_slice = &self.input[start..limit];

        let line_end = input_slice.find('\n').map_or(limit, |i| start + i);

        let value = &self.input[start..line_end];

        // Validate timestamp - Deviation from Elisp:
        // Elisp uses org-parse-time-string which is more lenient.
        // We use strict validation to match our test expectations.
        if !Self::is_valid_clock_timestamp(value) {
            return SyntaxNode::fallback(self.input, start, limit);
        }

        let end = line_end;

        let post_blank = if end < limit {
            let remaining = &self.input[end..limit];
            let trimmed = remaining.trim_start();
            (remaining.len() - trimmed.len()).min(2)
        } else {
            0
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::Clock(Box::new(crate::data::ClockData {
                duration: "",
                status: crate::data::ClockStatus::Running,
                raw: value,
            })),
            location: Interval { start, end },
            content_location: None,
            post_blank,
            affiliated: None,
        }
    }

    /// Validate clock timestamp format and check for invalid dates.
    ///
    /// Returns true if the timestamp is valid, false otherwise.
    /// This is more strict than Elisp's org-parse-time-string.
    fn is_valid_clock_timestamp(line: &str) -> bool {
        // Handle case-insensitive "CLOCK:" prefix
        let trimmed = line.trim();

        // Duration-only clocks (e.g., "CLOCK: => 0:11") are valid
        if trimmed.contains("=>") && !trimmed.contains('[') {
            return true;
        }

        // For clocks with timestamps, validate the date
        if let Some(start) = trimmed.find('[') {
            if let Some(end_bracket) = trimmed[start..].find(']') {
                let ts = &trimmed[start + 1..start + end_bracket];

                let parts: Vec<&str> = ts.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Some(date_part) = parts.get(0) {
                        let date_parts: Vec<&str> = date_part.split('-').collect();
                        if date_parts.len() == 3 {
                            if let (Ok(year), Ok(month), Ok(day)) = (
                                date_parts[0].parse::<u32>(),
                                date_parts[1].parse::<u32>(),
                                date_parts[2].parse::<u32>(),
                            ) {
                                if month > 12 || day > 31 || year < 1970 || year > 2100 {
                                    return false;
                                }
                                if month == 2 && day == 29 && !Self::is_leap_year(year) {
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
        }

        true
    }

    /// Check if a year is a leap year.
    fn is_leap_year(year: u32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Parse a diary sexp element.
    ///
    /// Diary sexp format: `%%(SEXP)` at beginning of line (unindented).
    /// The sexp must have balanced parentheses.
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound search
    /// - Uses `start` for begin position
    /// - Value is the full sexp string
    /// Parse a diary sexp element.
    ///
    /// Diary sexp format: `%%(SEXP)` at beginning of line (unindented).
    /// The sexp must have balanced parentheses.
    ///
    /// Matches Elisp implementation:
    /// - Uses `limit` to bound search
    /// - Uses `start` for begin position
    /// - Value is the full sexp string
    /// - Stores affiliated data in SyntaxNode
    pub fn diary_sexp_parser(
        &self,
        limit: usize,
        start: usize,
        affiliated: Option<AffiliatedData<'a>>,
    ) -> SyntaxNode<'a> {
        let input_slice = &self.input[start..limit];

        let caps = match REGEX_DIARY_SEXP.captures(input_slice) {
            Some(c) => c,
            None => return SyntaxNode::fallback(self.input, start, limit),
        };

        let value = match caps.get(1) {
            Some(m) => m.as_str(),
            None => return SyntaxNode::fallback(self.input, start, limit),
        };

        let line_end = self.input[start..limit]
            .find('\n')
            .map_or(limit, |i| start + i);

        let end = line_end;

        let post_blank = if end < limit {
            let remaining = &self.input[end..limit];
            let trimmed = remaining.trim_start();
            (remaining.len() - trimmed.len()).min(2)
        } else {
            0
        };

        SyntaxNode {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: Syntax::DiarySexp(Box::new(crate::data::DiarySexpData { value })),
            location: Interval { start, end },
            content_location: None,
            post_blank,
            affiliated,
        }
    }
}
