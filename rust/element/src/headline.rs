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

lazy_static! {
    pub static ref REGEX_HEADLINE_SHORT: Regex = Regex::new(r"\*+\s").unwrap();
}

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

pub struct NodePropertyData<'a> {
    key: &'a str,
    value: &'a str,
}

pub struct Tag<'a>(&'a str);

pub enum TodoKeyword {
    TODO,
    DONE,
}

impl<'a> Parser<'a> {
    // TODO implement headline_parser
    pub fn headline_parser(&self) -> SyntaxNode<'a> {
        unimplemented!()
    }

    // TODO implement node_property_parser
    pub fn node_property_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
