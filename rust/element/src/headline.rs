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

//! Headlines and Sections
//! https://orgmode.org/worg/dev/org-syntax.html#Headlines_and_Sections
//!
//! A headline is defined as:
//!
//! STARS KEYWORD PRIORITY TITLE TAGS
//!
//! STARS is a string starting at column 0, containing at least one
//! asterisk and ended by a space character.  The number of asterisks
//! is used to define the level of the headline.
//!
//! KEYWORD is a TODO keyword (TODO or DONE).
//!
//! PRIORITY is a priority cookie, e.g. `[#A]`.
//!
//! TITLE can be made of any character but a new line.
//!
//! TAGS is made of words separated with colons, e.g. `:tag1:tag2:`.

use crate::data::{Interval, NodeId, Syntax, SyntaxNode, TimestampData};
use crate::parser::Parser;
use memchr::memchr;
use regex::Regex;

const ORG_CLOSED_STRING: &str = "CLOSED";
const ORG_DEADLINE_STRING: &str = "DEADLINE";
const ORG_SCHEDULED_STRING: &str = "SCHEDULED";

lazy_static! {
    pub static ref REGEX_HEADLINE_SHORT: Regex = Regex::new(r"^\*+\s").unwrap();

    pub static ref REGEX_HEADLINE_MULTILINE: Regex = Regex::new(r"(?m)^\*+\s").unwrap();

    /// Matches a line with planning info.
    /// Matched keyword is in group 1
    pub static ref REGEX_PLANNING_LINE: Regex = Regex::new(
        &format!(r"^[ \t]*((?:{}|{}|{}):)",
            ORG_CLOSED_STRING, ORG_DEADLINE_STRING, ORG_SCHEDULED_STRING ))
        .unwrap();

    /// Matches an entire property drawer.
    pub static ref REGEX_PROPERTY_DRAWER: Regex = Regex::new(
        r"^[ \t]*:PROPERTIES:[ \t]*\n(?:[ \t]*:\S+:(?: .*)?[ \t]*\n)*?[ \t]*:END:[ \t]*")
            .unwrap();

    pub static ref REGEX_CLOCK_LINE: Regex = Regex::new(r"(?i)^[ \t]*clock:").unwrap();
}

#[derive(Debug)]
pub struct HeadlineData<'a> {
    /// Non-nil if the headline has an archive tag.
    pub archivedp: bool,

    /// Headline's CLOSED reference, if any.
    pub closed: Option<TimestampData<'a>>,

    /// Non-nil if the headline has a comment keyword.
    pub commentedp: bool,

    /// Headline's DEADLINE reference, if any.
    pub deadline: Option<TimestampData<'a>>,

    /// Non-nil if the headline is a footnote section.
    pub footnote_section_p: bool,

    /// Reduced level of the headline (number of stars).
    pub level: usize,

    /// Number of blank lines between the headline and its contents.
    pub pre_blank: usize,

    /// Headline's priority, as a character.
    pub priority: usize,

    /// Non-nil if the headline contains a quote keyword.
    pub quotedp: bool,

    /// Raw headline text, without the stars and the tags.
    pub raw_value: &'a str,

    /// Headline's SCHEDULED reference, if any.
    pub scheduled: Option<TimestampData<'a>>,

    /// Headline's tags, if any.
    pub tags: Vec<Tag<'a>>,

    /// Parsed headline text, without the stars and the tags.
    pub title: &'a str,

    /// Byte range of `title` within the original input, for secondary-string
    /// object parsing.  `None` when the title is empty.
    pub title_location: Option<Interval>,

    /// Headline's TODO keyword, if any.
    pub todo_keyword: Option<TodoKeyword>,
}

#[derive(Debug)]
pub struct InlineTaskData<'a> {
    pub closed: Option<TimestampData<'a>>,
    pub deadline: Option<TimestampData<'a>>,
    pub level: usize,
    pub priority: usize,
    pub raw_value: &'a str,
    pub scheduled: Option<TimestampData<'a>>,
    pub tags: Vec<Tag<'a>>,
    pub title: &'a str,
    pub todo_keyword: Option<TodoKeyword>,
}

#[derive(Debug)]
pub struct NodePropertyData<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

#[derive(Debug)]
pub struct Tag<'a>(pub &'a str);

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum TodoKeyword {
    TODO,
    DONE,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Parse a headline at the current cursor position.
    ///
    /// Extracts the level (star count), optional TODO/DONE keyword,
    /// title text, and tags.  The headline node spans from the `*` line
    /// to the next headline at the same or higher level, or buffer end.
    #[inline]
    pub fn headline_parser(&mut self) -> NodeId {
        let begin = {
            self.cursor.goto_line_begin();
            self.cursor.pos()
        };

        let bytes = self.input.as_bytes();

        // Count leading stars → level.
        let mut level = 0;
        while begin + level < self.input.len() && bytes[begin + level] == b'*' {
            level += 1;
        }

        // Find end of the headline *line*.
        let line_end = memchr(b'\n', &self.input.as_bytes()[begin..])
            .map_or(self.input.len(), |i| begin + i);

        // The text after the stars and the space.
        let after_stars_start = (begin + level + 1).min(line_end);
        let after_stars = &self.input[after_stars_start..line_end];

        // Parse TODO/DONE keyword.
        let (todo_keyword, rest) = if after_stars.starts_with("TODO ") || after_stars == "TODO" {
            (Some(TodoKeyword::TODO), after_stars.get(5..).unwrap_or(""))
        } else if after_stars.starts_with("DONE ") || after_stars == "DONE" {
            (Some(TodoKeyword::DONE), after_stars.get(5..).unwrap_or(""))
        } else {
            (None, after_stars)
        };

        // Parse priority cookie [#A], [#B], etc.
        let (priority, rest) = rest
            .strip_prefix("[#")
            .and_then(|s| s.split_once(']'))
            .filter(|(cookie, _)| cookie.len() == 1 && cookie.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
            .map(|(cookie, after)| (cookie.chars().next().unwrap() as usize, after.trim_start()))
            .unwrap_or((0, rest));

        // Parse tags at end of line: look for `:tag1:tag2:` pattern.
        let rest_trimmed = rest.trim_end();
        let (raw_title, tags) = parse_headline_tags(rest_trimmed);

        // Strip COMMENT keyword from title if present.
        let commentedp = raw_title.starts_with("COMMENT ") || raw_title == "COMMENT";
        let title = raw_title.strip_prefix("COMMENT ").unwrap_or(raw_title);

        // Compute raw_value: everything between stars+space and line end.
        let raw_value = &self.input[after_stars_start..line_end];

        // Compute the byte range of `title` within `self.input` via pointer
        // arithmetic.  All intermediate string slices are derived from
        // `self.input`, so the pointer is valid.  The empty literal produced
        // for a COMMENT-only headline has `len == 0`, handled by `then`.
        let title_location = (!title.is_empty()).then(|| {
            let input_base = self.input.as_ptr() as usize;
            let start = title.as_ptr() as usize - input_base;
            Interval { start, end: start + title.len() }
        });

        // Find end of headline subtree: next headline at same or higher
        // level, or end of buffer.
        let content_start = (line_end + 1).min(self.input.len());
        let end = find_headline_end(self.input, content_start, level);

        // 15+ stars are inline tasks, not headlines.
        if level >= 15 {
            return self.arena.alloc(SyntaxNode {
                parent: None,
                children: Vec::new(),
                data: Syntax::InlineTask(Box::new(InlineTaskData {
                    closed: None,
                    deadline: None,
                    level,
                    priority,
                    raw_value,
                    scheduled: None,
                    tags,
                    title,
                    todo_keyword,
                })),
                location: Interval {
                    start: begin,
                    end: (line_end + 1).min(self.input.len()),
                },
                content_location: None,
                post_blank: 0,
                affiliated: None,
            });
        }

        let data = HeadlineData {
            archivedp: tags.iter().any(|t| t.0 == "ARCHIVE"),
            closed: None,
            commentedp,
            deadline: None,
            footnote_section_p: false,
            level,
            pre_blank: 0,
            priority,
            quotedp: false,
            raw_value,
            scheduled: None,
            tags,
            title,
            title_location,
            todo_keyword,
        };

        let content_location = if content_start < end {
            Some(Interval {
                start: content_start,
                end,
            })
        } else {
            None
        };

        self.arena.alloc(SyntaxNode {
            parent: None,
            children: Vec::new(),
            data: Syntax::Headline(Box::new(data)),
            location: Interval { start: begin, end },
            content_location,
            post_blank: 0,
            affiliated: None,
        })
    }

    /// Fallback: inline task parser (not yet implemented).
    #[inline]
    pub fn inlinetask_parser(&mut self, limit: usize, _raw_secondary_p: bool) -> NodeId {
        let start = self.cursor.pos();
        self.arena.alloc(SyntaxNode::fallback(self.input, start, limit))
    }

    /// Fallback: property drawer parser (not yet implemented).
    #[inline]
    pub fn property_drawer_parser(&mut self, limit: usize) -> NodeId {
        let start = self.cursor.pos();
        self.arena.alloc(SyntaxNode::fallback(self.input, start, limit))
    }

    /// Fallback: node property parser (not yet implemented).
    #[inline]
    pub fn node_property_parser(&mut self, limit: usize) -> NodeId {
        let start = self.cursor.pos();
        self.arena.alloc(SyntaxNode::fallback(self.input, start, limit))
    }
}

/// Find the end of a headline subtree: the byte position of the next
/// headline at the same or higher level, or the end of the buffer.
fn find_headline_end(input: &str, from: usize, level: usize) -> usize {
    let bytes = input.as_bytes();
    let mut pos = from;
    while pos < input.len() {
        // Check if this line starts a headline.
        if bytes[pos] == b'*' {
            let mut stars = 0;
            while pos + stars < input.len() && bytes[pos + stars] == b'*' {
                stars += 1;
            }
            // A headline at same or higher level ends this subtree.
            if stars <= level
                && pos + stars < input.len()
                && (bytes[pos + stars] == b' ' || bytes[pos + stars] == b'\t')
            {
                return pos;
            }
        }
        // Advance to next line.
        match memchr(b'\n', &bytes[pos..]) {
            Some(i) => pos += i + 1,
            None => return input.len(),
        }
    }
    input.len()
}

fn is_valid_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '@' || c == '#' || c == '%'
}

/// Parse tags from the end of a headline title string.
///
/// Returns `(title_without_tags, vec_of_tags)`.
fn parse_headline_tags<'a>(line: &'a str) -> (&'a str, Vec<Tag<'a>>) {
    if !line.ends_with(':') {
        return (line, vec![]);
    }

    let mut rev_iter = line.char_indices().rev();

    // Skip past the trailing ':'.
    match rev_iter.next() {
        Some((_, ':')) => {}
        _ => return (line, vec![]),
    }

    let mut rev_iter = rev_iter.peekable();
    while let Some((byte_pos, c)) = rev_iter.next() {
        if c == ':' {
            match rev_iter.peek() {
                Some((_, prev_c)) if prev_c.is_whitespace() => {
                    let tag_start = byte_pos + 1;
                    let tag_str = &line[tag_start..line.len() - 1];
                    let tags: Vec<Tag<'a>> = tag_str
                        .split(':')
                        .filter(|s| !s.is_empty())
                        .map(Tag)
                        .collect();
                    if !tags.is_empty() {
                        let title = line[..byte_pos].trim_end();
                        return (title, tags);
                    }
                    return (line, vec![]);
                }
                _ => {}
            }
        } else if !is_valid_tag_char(c) {
            return (line, vec![]);
        }
    }

    (line, vec![])
}
