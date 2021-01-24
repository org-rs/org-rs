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
//! A headline is defined as:
//!
//! STARS KEYWORD PRIORITY TITLE TAGS
//!
//! STARS is a string starting at column 0, containing at least one asterisk (and up to
//! org-inlinetask-min-level if org-inlinetask library is loaded) and ended by a space character. The
//! number of asterisks is used to define the level of the headline. It’s the sole compulsory part of
//! a headline.
//!
//! KEYWORD is a TODO keyword, which has to belong to the list defined in org-todo-keywords-1. Case is
//! significant.
//!
//! PRIORITY is a priority cookie, i.e. a single letter preceded by a hash sign # and enclosed within
//! square brackets.
//!
//! TITLE can be made of any character but a new line. Though, it will match after every other part
//! have been matched.
//!
//! TAGS is made of words containing any alpha-numeric character, underscore, at sign, hash sign or
//! percent sign, and separated with colons.
//!
//! Examples of valid headlines include:
//!
//!
//! *
//!
//! ** DONE
//!
//! *** Some e-mail
//!
//! **** TODO [#A] COMMENT Title :tag:a2%:
//!
//!
//! If the first word appearing in the title is “COMMENT”, the headline will be considered as
//! “commented”. Case is significant.
//!
//! If its title is org-footnote-section, it will be considered as a “footnote section”. Case is
//! significant.
//!
//! If “ARCHIVE” is one of its tags, it will be considered as “archived”. Case is significant.
//!
//! A headline contains directly one section (optionally), followed by any number of deeper level
//! headlines.
//!
//! A section contains directly any greater element or element. Only a headline can contain a section.
//! As an exception, text before the first headline in the document also belongs to a section.
//!
//! As an example, consider the following document:
//!
//! An introduction.
//!
//! * A Headline
//!
//! Some text.
//!
//! ** Sub-Topic 1
//!
//! ** Sub-Topic 2
//!
//! *** Additional entry
//!
//! Its internal structure could be summarized as:
//!
//! (document
//!  (section)
//!  (headline
//!   (section)
//!   (headline)
//!   (headline
//!    (headline))))
//!

use crate::data::{SyntaxNode, TimestampData};
use crate::parser::Parser;
use regex::Regex;

const ORG_CLOSED_STRING: &str = "CLOSED";
const ORG_DEADLINE_STRING: &str = "DEADLINE";
const ORG_SCHEDULED_STRING: &str = "SCHEDULED";

lazy_static! {
    pub static ref REGEX_HEADLINE_SHORT: Regex = Regex::new(r"^\*+\s").unwrap();

    // TODO document why is it needed and what are the consequences of using multiline regex
    pub static ref REGEX_HEADLINE_MULTILINE: Regex = Regex::new(r"(?m)^\*+\s").unwrap();

    /// Matches a line with planning info.
    /// Matched keyword is in group 1
    pub static ref REGEX_PLANNING_LINE: Regex = Regex::new(
        &format!(r"^[ \t]*((?:{}|{}|{}):)",
            ORG_CLOSED_STRING, ORG_DEADLINE_STRING, ORG_SCHEDULED_STRING ))
        .unwrap();

    /// Matches an entire property drawer
    /// Requires multiline match
    /// correspond to org-property-drawer-re in org.el
    pub static ref REGEX_PROPERTY_DRAWER: Regex = Regex::new(
        r"^[ \t]*:PROPERTIES:[ \t]*\n(?:[ \t]*:\S+:(?: .*)?[ \t]*\n)*?[ \t]*:END:[ \t]*")
            .unwrap();

    pub static ref REGEX_CLOCK_LINE: Regex = Regex::new(r"^[ \t]*CLOCK:").unwrap();

}

#[derive(Debug)]
pub struct HeadlineData<'a> {
    /// Non_nil if the headline has an archive tag (boolean).
    archivedp: bool,

    /// Headline's CLOSED reference, if any (timestamp object or nil)
    closed: Option<TimestampData<'a>>,

    /// Non_nil if the headline has a comment keyword (boolean).
    commentedp: bool,

    /// Headline's DEADLINE reference, if any (timestamp object or nil).
    deadline: Option<TimestampData<'a>>,

    /// Non_nil if the headline is a footnote section (boolean).
    footnote_section_p: bool,

    /// Reduced level of the headline (integer).
    level: usize,

    /// Number of blank lines between the headline
    /// and the first non_blank line of its contents (integer).
    pre_blank: usize,

    /// Headline's priority, as a character (integer).
    priority: usize,

    /// Non_nil if the headline contains a quote keyword (boolean).
    quotedp: bool,

    /// Raw headline's text, without the stars and the tags (string).
    raw_value: &'a str,

    /// Headline's SCHEDULED reference, if any (timestamp object or nil).
    scheduled: Option<TimestampData<'a>>,

    /// Headline's tags, if any, without
    /// the archive tag. (list of strings).
    tags: Vec<Tag<'a>>,

    /// Parsed headline's text, without the stars
    /// and the tags (secondary string).
    title: &'a str,

    /// Headline's TODO keyword without quote and comment
    /// strings, if any (string or nil).
    /// also used instead of todo-type
    todo_keyword: TodoKeyword,
}

#[derive(Debug)]
pub struct InlineTaskData<'a> {
    /// Inlinetask's CLOSED reference, if any (timestamp object or nil)
    closed: Option<TimestampData<'a>>,

    /// Inlinetask's DEADLINE reference, if any (timestamp object or nil).
    deadline: Option<TimestampData<'a>>,

    /// Reduced level of the inlinetask (integer).
    level: usize,

    /// Headline's priority, as a character (integer).
    priority: usize,

    /// Raw inlinetask's text, without the stars and the tags (string).
    raw_value: &'a str,

    /// Inlinetask's SCHEDULED reference, if any (timestamp object or nil).
    scheduled: Option<TimestampData<'a>>,

    /// Inlinetask's tags, if any (list of strings).
    tags: Vec<Tag<'a>>,

    /// Parsed inlinetask's text, without the stars
    /// and the tags (secondary string).
    title: &'a str,

    /// Inlinetask's TODO keyword, if any (string or nil).
    /// Type of inlinetask's TODO keyword, if any (symbol done, todo).
    todo_keyword: Option<TodoKeyword>,
}

// A planning is an element with the following pattern:
// HEADLINE
// PLANNING
//
// where HEADLINE is a headline element and PLANNING is a line filled with INFO parts, where each of them follows the pattern:
//
// KEYWORD: TIMESTAMP
//
// KEYWORD is either “DEADLINE”, “SCHEDULED” or “CLOSED”. TIMESTAMP is a timestamp object.
//
// In particular, no blank line is allowed between PLANNING and HEADLINE.

#[derive(Debug)]
pub struct NodePropertyData<'a> {
    key: &'a str,
    value: &'a str,
}

#[derive(Debug)]
pub struct Tag<'a>(&'a str);

#[derive(Debug)]
pub enum TodoKeyword {
    TODO,
    DONE,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    // TODO implement headline_parser
    pub fn headline_parser(&self) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement inlinetask_parser
    pub fn inlinetask_parser(&self, limit: usize, raw_secondary_p: bool) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement property_drawer_parser
    pub fn property_drawer_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement node_property_parser
    pub fn node_property_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
