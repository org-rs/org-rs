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

use crate::cursor::Cursor;
use crate::cursor::LinesMetric;
use crate::cursor::Metric;
use crate::data::{SyntaxNode, TimestampData};
use crate::parser::Parser;
use regex::Regex;
use std::borrow::Cow;

const ORG_CLOSED_STRING: &str = "CLOSED";
const ORG_DEADLINE_STRING: &str = "DEADLINE";
const ORG_SCHEDULED_STRING: &str = "SCHEDULED";

const ORG_FOOTNOTE_SECTION: &str = "Footnotes";

lazy_static! {


    /// Matches any of the 3 keywords, together with the time stamp.
    pub static ref REGEX_TIME_NOT_CLOCK : Regex = Regex::new(r"((?:CLOSED|DEADLINE|SCHEDULED):) *[[<]([^]>]+)[]>]").unwrap();

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
        r"(?i)^[ \t]*:PROPERTIES:[ \t]*\n(?:[ \t]*:\S+:(?: .*)?[ \t]*\n)*?[ \t]*:END:[ \t]*")
            .unwrap();

    pub static ref REGEX_CLOCK_LINE: Regex = Regex::new(r"(?i)^[ \t]*CLOCK:").unwrap();

    /// Matches any of the TODO state keywords.
    /// TODO parametrize
    pub static ref REGEX_TODO: Regex = Regex::new(r"(?i)(TODO|DONE)[ \t]").unwrap();


    /// TODO parametrize
    /// check how org-done-keywords are set
    pub static ref REGEX_TODO_DONE: Regex = Regex::new(r"(?i)DONE").unwrap();


    pub static ref REGEX_HEADLINE_PRIORITY: Regex = Regex::new(r"\[#.\][ \t]*").unwrap();

    // org-comment-string
    pub static ref REGEX_HEADLINE_COMMENT: Regex = Regex::new(r"(?i)COMMENT").unwrap();

    pub static ref REGEX_HEDLINE_TAGS: Regex = Regex::new(r"[ \t]+(:[[:alnum:]_@#%:]+:)[ \t]*$").unwrap();




}

pub struct HeadlineMetric(());

impl Metric for HeadlineMetric {
    /// Return true if offset is on a headline.
    /// Any position on headline is considered to be a boundary
    fn is_boundary(s: &str, offset: usize) -> bool {
        let beg = if offset == 0 || LinesMetric::is_boundary(s, offset) {
            offset
        } else {
            match LinesMetric::prev(s, offset) {
                Some(p) => p,
                None => 0,
            }
        };

        let end = match LinesMetric::next(s, offset) {
            Some(p) => p,
            None => s.len(),
        };

        REGEX_HEADLINE_SHORT.is_match(&s[beg..end])
    }

    fn prev(s: &str, offset: usize) -> Option<usize> {
        let mut pos = offset;

        loop {
            if pos == 0 {
                return None;
            }

            let end = if LinesMetric::is_boundary(s, pos) {
                pos
            } else {
                match LinesMetric::prev(s, pos) {
                    Some(p) => p,
                    // No previos line - no previous headline
                    None => return None,
                }
            };

            let beg = match LinesMetric::prev(s, end) {
                Some(p) => p,
                None => 0,
            };

            if REGEX_HEADLINE_SHORT.is_match(&s[beg..end]) {
                return Some(beg);
            } else {
                pos = beg;
            }
        }
    }

    /// Possibly finds beginning of the next headline
    /// corresponds to `outline-next-heading` in emacs
    /// If next headline is found returns it's start position
    fn next(s: &str, offset: usize) -> Option<usize> {
        let beg = if HeadlineMetric::is_boundary(s, offset) {
            match LinesMetric::next(s, offset) {
                Some(p) => p,
                None => return None,
            }
        } else {
            offset
        };

        match REGEX_HEADLINE_MULTILINE.find(&s[beg..]) {
            Some(p) => Some(beg + p.start()),
            None => None,
        }
    }
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
    priority: Option<Priority>,

    /// Non_nil if the headline contains a quote keyword (boolean).
    quotedp: bool,

    /// Raw headline's text, without the stars and the tags (string).
    raw_value: Cow<'a, str>,

    /// Headline's SCHEDULED reference, if any (timestamp object or nil).
    scheduled: Option<TimestampData<'a>>,

    /// Headline's tags, if any, without
    /// the archive tag. (list of strings).
    tags: Vec<Tag<'a>>,

    /// Parsed headline's text, without the stars
    /// and the tags (secondary string).
    title: Option<Cow<'a, str>>,

    /// Headline's TODO keyword without quote and comment
    /// strings, if any (string or nil).
    todo_keyword: Option<TodoKeyword<'a>>,

    todo_type: Option<TodoType>,

    properties: NodePropertyData<'a>,
}

#[derive(Debug, PartialEq)]
pub struct Priority(char);

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

#[derive(Debug, PartialEq)]
pub struct NodePropertyData<'a> {
    key: Cow<'a, str>,
    value: Cow<'a, str>,
}

#[derive(Debug, PartialEq)]
pub struct Tag<'a>(Cow<'a, str>);

impl<'a> Tag<'a> {
    fn new(s: &'a str) -> Tag<'a> {
        Tag(Cow::from(s))
    }
}

macro_rules! tag {
    ($s:literal) => {
        &Tag::new($s)
    };
}

#[derive(Debug)]
pub struct TagParseError;

#[derive(Debug, PartialEq)]
pub struct TodoKeyword<'a>(Cow<'a, str>);

// TODO this have to be defined by user set vaiable
impl<'a> TodoKeyword<'a> {
    fn is_done(&self) -> bool {
        REGEX_TODO_DONE.find(&self.0).is_some()
    }
}

pub enum TodoType {
    TODO,
    DONE,
}

impl<'a> Parser<'a> {
    /// Parse a headline.
    /// Return a list whose CAR is `headline' and CDR is a plist
    /// containing `:raw-value', `:title', `:begin', `:end',
    /// `:pre-blank', `:contents-begin' and `:contents-end', `:level',
    /// `:priority', `:tags', `:todo-keyword',`:todo-type', `:scheduled',
    /// `:deadline', `:closed', `:archivedp', `:commentedp'
    /// `:footnote-section-p', `:post-blank' and `:post-affiliated'
    /// keywords.
    ///
    /// The plist also contains any property set in the property drawer,
    /// with its name in upper cases and colons added at the
    /// beginning (e.g., `:CUSTOM_ID').
    ///
    /// LIMIT is a buffer position bounding the search.
    ///
    /// When RAW-SECONDARY-P is non-nil, headline's title will not be
    /// parsed as a secondary string, but as a plain string instead.
    ///
    /// Assume point is at beginning of the headline."

    pub fn headline_parser(&self, limit: usize, raw_secondary_p: bool) -> SyntaxNode<'a> {
        let mut cursor = self.cursor.borrow_mut();
        let begin = cursor.pos();

        let level = cursor.skip_chars_forward("*", Some(limit));
        cursor.skip_chars_forward(" \t", Some(limit));

        let todo = match cursor.capturing_at(&*REGEX_TODO) {
            None => None,
            Some(m) => {
                let m0 = m.get(0).unwrap();
                let m1 = m.get(1).unwrap();
                let res =
                    Cow::from(&self.input[cursor.pos() + m1.start()..cursor.pos() + m1.end()]);
                cursor.inc(m0.end());
                cursor.skip_chars_forward(" \t", Some(limit));
                Some(TodoKeyword(res))
            }
        };

        let todo_type: Option<TodoType> = match todo {
            Some(t) => {
                if t.is_done() {
                    Some(TodoType::DONE)
                } else {
                    Some(TodoType::TODO)
                }
            }
            None => None,
        };

        let priority: Option<Priority> = match cursor.looking_at(&*REGEX_HEADLINE_PRIORITY) {
            None => None,
            Some(m) => {
                let c = &self.input[cursor.pos() + m.start() + 2..]
                    .chars()
                    .next()
                    .expect("Must be at char boundary");
                cursor.inc(m.end());
                Some(Priority(*c))
            }
        };

        let commentedp: bool = match cursor.looking_at(&*REGEX_HEADLINE_COMMENT) {
            None => false,
            Some(m) => {
                cursor.inc(m.end());
                true
            }
        };

        let title_start = cursor.pos();
        let line_end_pos = cursor.line_end_position(None);
        let tags: Vec<Tag> =
            match cursor.re_search_forward(&*REGEX_HEDLINE_TAGS, Some(line_end_pos)) {
                None => vec![],
                Some(m) => (&self.input[m.start + 1..m.end])
                    .split(':')
                    .map(|t| Tag(Cow::from(t)))
                    .collect(),
            };

        let title_end = cursor.pos();
        let raw_value = Cow::from(&self.input[title_start..title_end]);

        // TODO add tests
        let archivedp = tags.contains(tag!("ARCHIVED"));
        let footnote_section_p = raw_value == ORG_FOOTNOTE_SECTION;
        let standard_props = self.get_node_properties();
        let time_props = self.get_time_poperties();

        let mut saved_excursion = cursor.pos();
        // (end (min (save-excursion (org-end-of-subtree t t)) limit))
        let end = std::cmp::min(self.end_of_subtree(), limit);
        cursor.set(saved_excursion);

        cursor.goto_next_line();
        cursor.skip_chars_forward("\r\t\n", Some(end));
        let contents_begin = if cursor.pos() != end {
            Some(cursor.line_beginning_position(None))
        } else {
            None
        };
        cursor.set(saved_excursion);

        cursor.set(end);
        // cursor.skip_chars_backward(" \r\t\n");
        let contents_end = cursor.line_beginning_position(Some(2));

        cursor.set(begin);
        unimplemented!()

        //       (let ((headline
        // 	     (list 'headline
        // 		   (nconc
        // 		    (list :raw-value raw-value
        // 			  :begin begin
        // 			  :end end
        // 			  :pre-blank
        // 			  (if (not contents-begin) 0
        // 			    (1- (count-lines begin contents-begin)))
        // 			  :contents-begin contents-begin
        // 			  :contents-end contents-end
        // 			  :level level
        // 			  :priority priority
        // 			  :tags tags
        // 			  :todo-keyword todo
        // 			  :todo-type todo-type
        // 			  :post-blank
        // 			  (if contents-end
        // 			      (count-lines contents-end end)
        // 			    (1- (count-lines begin end)))
        // 			  :footnote-section-p footnote-section-p
        // 			  :archivedp archivedp
        // 			  :commentedp commentedp
        // 			  :post-affiliated begin)
        // 		    time-props
        // 		    standard-props))))
        // 	(org-element-put-property
        // 	 headline :title
        // 	 (if raw-secondary-p raw-value
        // 	   (org-element--parse-objects
        // 	    (progn (goto-char title-start)
        // 		   (skip-chars-forward " \t")
        // 		   (point))
        // 	    (progn (goto-char title-end)
        // 		   (skip-chars-backward " \t")
        // 		   (point))
        // 	    nil
        // 	    (org-element-restriction 'headline)
        // 	    headline)))))))
        //
    }

    /// Goto to the end of a subtree.
    /// (defun org-end-of-subtree (&optional invisible-ok to-heading)
    fn end_of_subtree(&self) -> usize {
        //  (org-back-to-heading invisible-ok)
        //  (let ((first t)
        //	(level (funcall outline-level)))
        //    (if (and (derived-mode-p 'org-mode) (< level 1000))
        //	;; A true heading (not a plain list item), in Org
        //	;; This means we can easily find the end by looking
        //	;; only for the right number of stars.  Using a regexp to do
        //	;; this is so much faster than using a Lisp loop.
        //	(let ((re (concat "^\\*\\{1," (int-to-string level) "\\} ")))
        //	  (forward-char 1)
        //	  (and (re-search-forward re nil 'move) (beginning-of-line 1)))
        //      ;; something else, do it the slow way
        //      (while (and (not (eobp))
        //		  (or first (> (funcall outline-level) level)))
        //	(setq first nil)
        //	(outline-next-heading)))
        //    (unless to-heading
        //      (when (memq (preceding-char) '(?\n ?\^M))
        //	;; Go to end of line before heading
        //	(forward-char -1)
        //	(when (memq (preceding-char) '(?\n ?\^M))
        //	  ;; leave blank line before heading
        //	  (forward-char -1)))))
        //  (point))
        unimplemented!();
    }

    /// Return node properties associated to headline at point.
    /// Upcase property names.  It avoids confusion between properties
    /// obtained through property drawer and default properties from the
    /// parser (e.g. `:end' and :END:).  Return value is a plist."
    /// (defun org-element--get-node-properties ()
    fn get_node_properties(&self) -> Vec<NodePropertyData> {
        //   (save-excursion
        //     (forward-line)
        //     (when (looking-at-p org-planning-line-re) (forward-line))
        //     (when (looking-at org-property-drawer-re)
        //       (forward-line)
        //       (let ((end (match-end 0)) properties)
        // 	(while (< (line-end-position) end)
        // 	  (looking-at org-property-re)
        // 	  (push (match-string-no-properties 3) properties)
        // 	  (push (intern (concat ":" (upcase (match-string 2)))) properties)
        // 	  (forward-line))
        // 	properties))))

        unimplemented!()
    }

    /// Return time properties associated to headline at point.
    /// Return value is a plist."
    /// (defun org-element--get-time-properties ()
    fn get_time_poperties(&self) {

        //  (save-excursion
        //    (when (progn (forward-line) (looking-at org-planning-line-re))
        //      (let ((end (line-end-position)) plist)
        //	(while (re-search-forward org-keyword-time-not-clock-regexp end t)
        //	  (goto-char (match-end 1))
        //	  (skip-chars-forward " \t")
        //	  (let ((keyword (match-string 1))
        //		(time (org-element-timestamp-parser)))
        //	    (cond ((equal keyword org-scheduled-string)
        //		   (setq plist (plist-put plist :scheduled time)))
        //		  ((equal keyword org-deadline-string)
        //		   (setq plist (plist-put plist :deadline time)))
        //		  (t (setq plist (plist-put plist :closed time))))))
        //	plist))))

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

mod test {

    use crate::cursor::Cursor;
    use crate::headline::HeadlineMetric;

    #[test]
    fn headline_boundary() {
        let rope = "Some text\n**** headline\nNot headline again";
        let mut cursor = Cursor::new(&rope, 0);

        assert!(!cursor.is_boundary::<HeadlineMetric>());

        cursor.set(4);
        assert!(!cursor.is_boundary::<HeadlineMetric>());
        assert_eq!(4, cursor.pos());

        cursor.set(15);
        assert!(cursor.is_boundary::<HeadlineMetric>());

        cursor.set(10);
        assert!(cursor.is_boundary::<HeadlineMetric>());
        assert_eq!(10, cursor.pos());
    }

    #[test]
    fn next_headline() {
        let string = "Some text\n** headline\nAnother line\n** another headline\n";
        let mut cursor = Cursor::new(&string, 0);
        assert_eq!(Some(10), cursor.at_or_next::<HeadlineMetric>());
        assert_eq!(10, cursor.pos());
        assert_eq!(Some(35), cursor.next::<HeadlineMetric>());
        assert_eq!(35, cursor.pos());
        assert_eq!(None, cursor.next::<HeadlineMetric>());

        let string2 = "* 1\n* 2\n* 3\n* 4\n* 5";
        cursor = Cursor::new(&string2, 0);
        assert_eq!(Some(0), cursor.at_or_next::<HeadlineMetric>());

        assert_eq!(Some(4), cursor.next::<HeadlineMetric>());
        assert_eq!(Some(8), cursor.next::<HeadlineMetric>());
        assert_eq!(Some(12), cursor.next::<HeadlineMetric>());
        assert_eq!(Some(16), cursor.next::<HeadlineMetric>());
        assert_eq!(16, cursor.pos());
    }

    #[test]
    fn prev_headline() {
        let string = "Some text\n** headline\nAnother line\n** another headline\n* ";
        let mut cursor = Cursor::new(&string, string.len());

        assert_eq!(Some(string.len()), cursor.at_or_prev::<HeadlineMetric>());
        assert_eq!(string.len(), cursor.pos());

        assert_eq!(Some(35), cursor.prev::<HeadlineMetric>());
        assert_eq!(35, cursor.pos());
        assert_eq!(Some(10), cursor.prev::<HeadlineMetric>());
        assert_eq!(10, cursor.pos());
        assert_eq!(None, cursor.prev::<HeadlineMetric>());

        let string2 = "* 1\n* 2\n* 3\n* 4\n* 5\n";
        cursor = Cursor::new(&string2, string2.len());
        assert_eq!(Some(16), cursor.prev::<HeadlineMetric>());
        assert_eq!(Some(12), cursor.prev::<HeadlineMetric>());
        assert_eq!(Some(8), cursor.prev::<HeadlineMetric>());
        assert_eq!(Some(4), cursor.prev::<HeadlineMetric>());
        assert_eq!(Some(0), cursor.prev::<HeadlineMetric>());
        assert_eq!(None, cursor.prev::<HeadlineMetric>());
    }

}
