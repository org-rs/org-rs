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

use crate::data::{BumpVec, Interval, NodeId, Syntax, SyntaxNode, TimestampData};
use crate::parser::Parser;
use memchr::memchr;

/// Packed bitflags for [`HeadlineData`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadlineFlags(u8);

impl HeadlineFlags {
    const ARCHIVEDP: u8 = 0b0001;
    const COMMENTEDP: u8 = 0b0010;
    const FOOTNOTE_SECTION_P: u8 = 0b0100;
    const QUOTEDP: u8 = 0b1000;

    #[inline]
    pub fn new(archivedp: bool, commentedp: bool, footnote_section_p: bool, quotedp: bool) -> Self {
        let mut f = 0;
        if archivedp {
            f |= Self::ARCHIVEDP;
        }
        if commentedp {
            f |= Self::COMMENTEDP;
        }
        if footnote_section_p {
            f |= Self::FOOTNOTE_SECTION_P;
        }
        if quotedp {
            f |= Self::QUOTEDP;
        }
        HeadlineFlags(f)
    }

    #[inline]
    pub fn archivedp(self) -> bool {
        self.0 & Self::ARCHIVEDP != 0
    }
    #[inline]
    pub fn commentedp(self) -> bool {
        self.0 & Self::COMMENTEDP != 0
    }
    #[inline]
    pub fn footnote_section_p(self) -> bool {
        self.0 & Self::FOOTNOTE_SECTION_P != 0
    }
    #[inline]
    pub fn quotedp(self) -> bool {
        self.0 & Self::QUOTEDP != 0
    }
}

#[derive(Debug)]
pub struct HeadlineData<'a, 'b> {
    /// Packed flags.
    pub flags: HeadlineFlags,

    /// Headline's CLOSED reference, if any.
    pub closed: Option<TimestampData<'a>>,

    /// Headline's DEADLINE reference, if any.
    pub deadline: Option<TimestampData<'a>>,

    /// Reduced level of the headline (number of stars).
    pub level: usize,

    /// Number of blank lines between the headline and its contents.
    pub pre_blank: usize,

    /// Headline's priority, as a character.
    pub priority: usize,

    /// Raw headline text, without the stars and the tags.
    pub raw_value: &'a str,

    /// Headline's SCHEDULED reference, if any.
    pub scheduled: Option<TimestampData<'a>>,

    /// Headline's tags, if any.
    pub tags: BumpVec<'b, Tag<'a>>,

    /// Parsed headline text, without the stars and the tags.
    pub title: &'a str,

    /// Byte range of `title` within the original input, for secondary-string
    /// object parsing.  `None` when the title is empty.
    pub title_location: Option<Interval>,

    /// Inline objects parsed from the headline title at Object granularity.
    /// Stored here rather than in `SyntaxNode::children` because the title is
    /// a secondary string (Emacs `:title` property), not body content
    /// (`org-element-contents`).  Body children are Sections and sub-Headlines
    /// only.
    pub title_objects: BumpVec<'b, NodeId>,

    /// Headline's TODO keyword, if any.
    pub todo_keyword: Option<TodoKeyword>,
}

impl<'a, 'b> HeadlineData<'a, 'b> {
    #[inline]
    pub fn archivedp(&self) -> bool {
        self.flags.archivedp()
    }
    #[inline]
    pub fn commentedp(&self) -> bool {
        self.flags.commentedp()
    }
    #[inline]
    pub fn footnote_section_p(&self) -> bool {
        self.flags.footnote_section_p()
    }
    #[inline]
    pub fn quotedp(&self) -> bool {
        self.flags.quotedp()
    }

    /// Parse tags from the end of a trimmed headline title string.
    ///
    /// Returns `(title_without_tags, vec_of_tags)`.
    pub fn parse_tags<'s, 'bump>(
        line: &'s str,
        bump: &'bump bumpalo::Bump,
    ) -> (&'s str, BumpVec<'bump, Tag<'s>>) {
        if !line.ends_with(':') {
            return (line, BumpVec::new_in(bump));
        }

        let mut rev_iter = line.char_indices().rev();

        match rev_iter.next() {
            Some((_, ':')) => {}
            _ => return (line, BumpVec::new_in(bump)),
        }

        let mut rev_iter = rev_iter.peekable();
        while let Some((byte_pos, c)) = rev_iter.next() {
            if c == ':' {
                match rev_iter.peek() {
                    Some((_, prev_c)) if prev_c.is_whitespace() => {
                        let tag_start = byte_pos + 1;
                        let tag_str = &line[tag_start..line.len() - 1];
                        let mut tags: BumpVec<'bump, Tag<'s>> = BumpVec::new_in(bump);
                        for s in tag_str.split(':').filter(|s| !s.is_empty()) {
                            tags.push(Tag(s));
                        }
                        if !tags.is_empty() {
                            let title = line[..byte_pos].trim_end();
                            return (title, tags);
                        }
                        return (line, BumpVec::new_in(bump));
                    }
                    _ => {}
                }
            } else if !c.is_alphanumeric() && c != '_' && c != '@' && c != '#' && c != '%' {
                return (line, BumpVec::new_in(bump));
            }
        }

        (line, BumpVec::new_in(bump))
    }
}

#[derive(Debug)]
pub struct InlineTaskData<'a, 'b> {
    pub closed: Option<TimestampData<'a>>,
    pub deadline: Option<TimestampData<'a>>,
    pub level: usize,
    pub priority: usize,
    pub raw_value: &'a str,
    pub scheduled: Option<TimestampData<'a>>,
    pub tags: BumpVec<'b, Tag<'a>>,
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

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    /// Parse a headline at the current cursor position.
    ///
    /// Extracts the level (star count), optional TODO/DONE keyword,
    /// title text, and tags.  The headline node spans from the `*` line
    /// to the next headline at the same or higher level, or buffer end.
    #[inline]
    pub fn headline_parser(&mut self, limit: usize) -> NodeId {
        let begin = {
            self.cursor.goto_line_begin();
            self.cursor.pos()
        };

        let bytes = self.input.as_bytes();

        let mut level = 0;
        while begin + level < self.input.len() && bytes[begin + level] == b'*' {
            level += 1;
        }

        let line_end =
            memchr(b'\n', &self.input.as_bytes()[begin..]).map_or(self.input.len(), |i| begin + i);

        let after_stars_start = (begin + level + 1).min(line_end);
        let after_stars = &self.input[after_stars_start..line_end];

        let (todo_keyword, rest) = if after_stars.starts_with("TODO ") || after_stars == "TODO" {
            (Some(TodoKeyword::TODO), after_stars.get(5..).unwrap_or(""))
        } else if after_stars.starts_with("DONE ") || after_stars == "DONE" {
            (Some(TodoKeyword::DONE), after_stars.get(5..).unwrap_or(""))
        } else {
            (None, after_stars)
        };

        let (priority, rest) = rest
            .strip_prefix("[#")
            .and_then(|s| s.split_once(']'))
            .filter(|(cookie, _)| {
                cookie.len() == 1
                    && cookie
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic())
            })
            .map(|(cookie, after)| (cookie.chars().next().unwrap() as usize, after.trim_start()))
            .unwrap_or((0, rest));

        let rest_trimmed = rest.trim_end();
        let (raw_title, tags) = HeadlineData::parse_tags(rest_trimmed, self.bump);

        let commentedp = raw_title.starts_with("COMMENT ") || raw_title == "COMMENT";
        let title = raw_title.strip_prefix("COMMENT ").unwrap_or(raw_title);

        let raw_value = &self.input[after_stars_start..line_end];

        let title_location = (!title.is_empty()).then(|| {
            let input_base = self.input.as_ptr() as usize;
            let start = title.as_ptr() as usize - input_base;
            Interval {
                start,
                end: start + title.len(),
            }
        });

        let content_start = (line_end + 1).min(self.input.len());
        let end = self.cursor.find_headline_end(content_start, level, limit);

        if level >= 15 {
            let heading_end = (line_end + 1).min(self.input.len());
            let (it_end, content_location) = match self.cursor.inlinetask_end_at(end) {
                Some(after_end) => {
                    let content = (content_start < end).then_some(Interval {
                        start: content_start,
                        end,
                    });
                    (after_end, content)
                }
                None => (heading_end, None),
            };
            return self.arena.alloc(SyntaxNode {
                parent: None,
                children: BumpVec::new_in(self.bump),
                data: Syntax::InlineTask(self.bump.alloc(InlineTaskData {
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
                    end: it_end,
                },
                content_location,
                post_blank: 0,
                affiliated: None,
            });
        }

        let data = HeadlineData {
            flags: HeadlineFlags::new(
                tags.iter().any(|t| t.0 == "ARCHIVE"),
                commentedp,
                false,
                false,
            ),
            closed: None,
            deadline: None,
            level,
            pre_blank: 0,
            priority,
            raw_value,
            scheduled: None,
            tags,
            title,
            title_location,
            title_objects: BumpVec::new_in(self.bump),
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
            children: BumpVec::new_in(self.bump),
            data: Syntax::Headline(self.bump.alloc(data)),
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
        self.arena
            .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump))
    }

    /// Fallback: property drawer parser (not yet implemented).
    #[inline]
    pub fn property_drawer_parser(&mut self, limit: usize) -> NodeId {
        let start = self.cursor.pos();
        self.arena
            .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump))
    }

    /// Fallback: node property parser (not yet implemented).
    #[inline]
    pub fn node_property_parser(&mut self, limit: usize) -> NodeId {
        let start = self.cursor.pos();
        self.arena
            .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump))
    }
}
