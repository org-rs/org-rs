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

//! Affiliated Keywords
//! https://orgmode.org/worg/dev/org-syntax.html#Affiliated_keywords
//!
//! With the exception of some elements every other element type
//! can be assigned attributes:
//!
//! Elemets that can NOT have affiliated keywords:
//! Items, Table Rows , Node Properties, Headlines
//! Sections, Planning lines, Property Drawers, Clocks and Inlinetasks
//!
//! This is done by adding specific keywords, named “affiliated keywords”,
//! just above the element considered, no blank line allowed.
//!
//! Affiliated keywords are built upon one of the following patterns:
//!
//! “#+KEY: VALUE” - Regular
//! “#+KEY[OPTIONAL]: VALUE” - Dual
//! “#+ATTR_BACKEND: VALUE”  - Exported Attribute
//!
//! KEY is either  “CAPTION”, “HEADER”, “NAME”, “PLOT” or “RESULTS” string.
//!
//! BACKEND is a string constituted of alpha-numeric characters, hyphens or underscores.
//!
//! OPTIONAL and VALUE can contain any character but a new line.
//! Only “CAPTION” and “RESULTS” keywords can have an optional value.
//!
//! An affiliated keyword can appear more than once if KEY is either “CAPTION” or “HEADER”
//! or if its pattern is “#+ATTR_BACKEND: VALUE”.
//!
//! “CAPTION” keyword can contain objects in both VALUE and OPTIONAL fileds.

use crate::cursor::REGEX_EMPTY_LINE;
use crate::data::StringOrObject;
use crate::data::SyntaxT;
use crate::parser::Parser;
use regex::{Match, Regex};
use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;

lazy_static! {

   /// Regexp matching any affiliated keyword
   ///
   /// This is different from original implementation for several reasons:
   ///
   /// ERE regexes support explicit numbering for capture groups,
   /// which also can be repetetive. Rust's regex is PCRE, and does
   /// not have such feature.
   ///
   /// Secondly, it uses different capture group arrangement to
   /// simplify parsing and serialization of the keywords
   ///
   /// Thirdly, translation from old keywords already built-in
   ///
   /// Capture groups are named after each Keyword respectively.
   ///
   /// If you want to use group numbers:
   /// CAPTION keyword is captured to group 1
   /// RESULTS keyword is captured to group 2
   ///
   /// Secondary value of CAPTION or RESULTS is captured to group 3
   ///
   /// HEADER keyword is captured to group 4
   /// PLOT keyword is captured to group 5
   /// NAME keyword is captured to group 6
   /// ATTR_ exported attribute is captured to group 7
   ///
   /// Warning! If you add more keywords then you must update this regex!
   /// Original elisp implementation dynamically creates this regex based on
   /// definitions lists of dual,regular and attribute keywords.
   /// While this is possible to do in rust, and maybe it will be required in
   /// the future, for now due laziness and lack of time static regex will be used.
   ///
   /// elisp: `org-element--affiliated-re`
   pub static ref REGEX_AFFILIATED: Regex = Regex::new(
           &format!(
              r"(?i)^[ \t]*{}|{}|{}|{}|{}[ \t]*",
              r"#\+(?:(?:(?P<CAPTION>CAPTION)|(?P<RESULTS>RESULTS?))(?:\[(?P<SECONDARY>.*)\])?",   // DUAL
              r"(?P<HEADER>HEADERS?)",
              r"(?P<PLOT>PLOT)",
              r"(?P<NAME>(?:DATA|LABEL|NAME|RESNAME|(?:S(?:OURC|RCNAM)|TBLNAM)E))",
              r"(?P<ATTR>ATTR_[-_A-Za-z0-9]+)):")
       ).unwrap();
}

/// Since CAPTION is both DUAL and PARSED DualVal has to be able to store Strings or StringOrObject
#[derive(Default, Debug, PartialEq)]
pub struct DualVal<T> {
    pub value: T,
    pub secondary: Option<T>,
}

/// List of affiliated keywords as strings.
/// elisp: `defconst org-element-affiliated-keywords`
/// Capabilities:
/// DUAL - can have optional secontary value
/// PARSED - value can be either string or an Object
/// MULTI: can occur more than once in an element.

#[derive(Debug, PartialEq)]
pub struct AffiliatedData<'a> {
    /// DUAL, PARSED, MULTI
    pub caption: Vec<DualVal<StringOrObject<'a>>>,
    /// MULTI
    pub header: Vec<Cow<'a, str>>,

    /// No special capabilities
    pub name: Option<Cow<'a, str>>,

    /// No special capabilities
    pub plot: Option<Cow<'a, str>>,

    /// DUAL
    pub results: Option<DualVal<Cow<'a, str>>>,

    /// MULTI
    pub attr: HashMap<String, Vec<Cow<'a, str>>>,
}

impl<'a> Default for AffiliatedData<'a> {
    fn default() -> AffiliatedData<'a> {
        AffiliatedData {
            caption: vec![],
            header: vec![],
            name: None,
            plot: None,
            results: None,
            attr: HashMap::new(),
        }
    }
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    /// Collect affiliated keywords from point down to LIMIT.
    ///
    /// Most elements can have affiliated keywords.  When looking for an
    /// element beginning, we want to move before them, as they belong to
    /// that element, and, in the meantime, collect information they give
    /// into appropriate properties.  Hence the following function.
    ///
    /// The following scenarios are possible.
    /// - Happy path:
    ///   Return a tuple whose first element is the position at the first of affiliated keyword and
    ///   second is `Some(AffiliatedData)`. Cursor is moved to the beginning of the first line
    ///   after affiliated keywords. Exception is when cursor reached the limit
    ///   and stopped on a non-empty line - cursor remains there.
    ///
    /// - Short circuit: if element doesn't start at the beginning of
    ///   the line (e.g., a paragraph starting an item), first tuple element is current
    ///   position of the cursor and second is None.
    ///   elisp `defun org-element--collect-affiliated-keywords (limit)`
    ///
    /// NB: it looks like this function parses objects ignoring parser granularity settings
    ///
    /// "FIXME: currently CAPTION's values are not parsed into objects and can only contain
    /// raw strings for now for the following reasons:
    ///
    /// - Original algorithm does not take into account granularity, and it is probably a bug.
    ///
    /// - It is unclear what should be the type of the field that stores the
    /// object since it can contain many objects.
    ///
    /// - It is unclear who should be the "parent" of these objects.  parse-objects function says
    /// that: "Eventually, if both ACC and PARENT are nil, the common parent is the list of
    /// objects itself." - It is hard to encode this into a type system, since in all other
    /// cases, apart from affiliated keywords, objects parents are nodes of syntax trees
    /// (ACC or PARENT)
    pub fn collect_affiliated_keywords(&self, limit: usize) -> (usize, Option<AffiliatedData>) {
        if !self.cursor.borrow().is_bol() {
            return (self.cursor.borrow().pos(), None);
        }
        let origin = self.cursor.borrow().pos();
        let _restrict = |that| SyntaxT::Keyword.can_contain(that);

        let mut output: AffiliatedData = Default::default();

        loop {
            let maybe_affiliated = capturing_at!(REGEX_AFFILIATED, self);
            let current_pos = self.cursor.borrow().pos();
            if current_pos >= limit || maybe_affiliated.is_none() {
                break;
            }
            let captures = maybe_affiliated.expect("Captures are expected here");

            let matched: (&str, Match) = REGEX_AFFILIATED
                .capture_names()
                .flatten()
                .filter(|n| n != &"SECONDARY")
                .filter_map(|n| Some((n, captures.name(n)?)))
                .next()
                .unwrap();

            let value_begin = match captures.name("SECONDARY") {
                None => self.cursor.borrow().pos() + matched.1.end() + 1,
                Some(sec) => self.cursor.borrow().pos() + sec.end() + 2,
            };

            let value_end = self.cursor.borrow_mut().line_end_position(None);

            let value = Cow::from(self.input[value_begin..value_end].trim());

            let secondary_value = match captures.name("SECONDARY") {
                None => None,
                Some(sec) => Some(Cow::from(sec.as_str().trim())),
            };

            match matched.0 {
                "CAPTION" => output.caption.push(DualVal {
                    value: StringOrObject::Raw(value),
                    secondary: secondary_value.map(StringOrObject::Raw),
                }),

                "RESULTS" => {
                    output.results = Some(DualVal {
                        value: value,
                        secondary: secondary_value,
                    })
                }

                "NAME" => output.name = Some(value),
                "PLOT" => output.plot = Some(value),
                "HEADER" => output.header.push(value),
                "ATTR" => {
                    let backend = captures.name("ATTR").unwrap().as_str().to_ascii_uppercase();
                    if let Some(vec) = output.attr.get_mut(backend.as_str()) {
                        vec.push(value);
                    } else {
                        output.attr.insert(backend, vec![value]);
                    }
                }

                _ => unreachable!(),
            }

            self.cursor.borrow_mut().goto_next_line();
        }

        // If affiliated keywords are orphaned: move back to first one.
        // They will be parsed as a paragraph.
        if looking_at!(REGEX_EMPTY_LINE, self).is_some() {
            self.cursor.borrow_mut().set(origin);
            return (origin, None);
        }

        return (origin, Some(output));
    }
}

mod test {
    use super::REGEX_AFFILIATED;
    use crate::affiliated::DualVal;
    use crate::cursor::{is_multiline_regex, Cursor};
    use crate::data::RepeaterType::CatchUp;
    use crate::data::StringOrObject;
    use crate::environment::DefaultEnvironment;
    use crate::parser::ParseGranularity;
    use crate::parser::Parser;
    use regex::Match;
    use std::borrow::Cow;
    use std::collections::HashMap;

    #[test]
    fn test_re() {
        let expected = r"(?i)^[ \t]*#\+(?:(?:(?P<CAPTION>CAPTION)|(?P<RESULTS>RESULTS?))(?:\[(?P<SECONDARY>.*)\])?|(?P<HEADER>HEADERS?)|(?P<PLOT>PLOT)|(?P<NAME>(?:DATA|LABEL|NAME|RESNAME|(?:S(?:OURC|RCNAM)|TBLNAM)E))|(?P<ATTR>ATTR_[-_A-Za-z0-9]+)):[ \t]*";
        assert_eq!(expected, REGEX_AFFILIATED.as_str());
    }

    #[test]
    fn affiliated_re() {
        assert!(!is_multiline_regex(REGEX_AFFILIATED.as_str()));
        let mut maybe_cap = REGEX_AFFILIATED.captures(r"  \n#+caPtion[GIT]: org-rs");
        assert!(maybe_cap.is_none());

        maybe_cap = REGEX_AFFILIATED.captures(r"   #+caPtion[GIT]: org-rs");
        let mut cap = maybe_cap.unwrap();
        assert_eq!("caPtion", cap.get(1).unwrap().as_str());
        assert_eq!("GIT", cap.get(3).unwrap().as_str());
        assert_eq!(None, cap.get(2));
        assert_eq!(None, cap.get(4));
        assert_eq!(None, cap.get(5));
        assert_eq!(None, cap.get(6));
        assert_eq!(None, cap.get(7));

        let dual_part = r"#+CAPTION: Orgmode";
        cap = REGEX_AFFILIATED.captures(dual_part).unwrap();
        assert_eq!("CAPTION", cap.get(1).unwrap().as_str());
        assert_eq!(None, cap.get(2));
        assert_eq!(None, cap.get(3));
        assert_eq!(None, cap.get(4));

        let single = r"#+RESNAME: someresult";
        cap = REGEX_AFFILIATED.captures(single).unwrap();
        assert_eq!("RESNAME", cap.get(6).unwrap().as_str());
        assert_eq!(None, cap.get(1));
        assert_eq!(None, cap.get(2));
        assert_eq!(None, cap.get(4));

        let attr = r"#+attr_html: :file filename.ext";
        cap = REGEX_AFFILIATED.captures(attr).unwrap();
        assert_eq!("attr_html", cap.get(7).unwrap().as_str());
        assert_eq!(None, cap.get(1));
        assert_eq!(None, cap.get(2));
        assert_eq!(None, cap.get(3));
    }

    #[test]
    fn looking_at_affiliated_re() {
        let caption_txt = " \n #+caPtion[GIT]: org-rs";
        let mut cursor = Cursor::new(caption_txt, 0);

        assert!(cursor.looking_at(&*REGEX_AFFILIATED).is_none());
        cursor.goto_next_line();
        assert_eq!(2, cursor.pos());
        assert!(cursor.looking_at(&*REGEX_AFFILIATED).is_some());
    }

    #[test]
    fn capturing_at_affiliated_re() {
        let mut text = String::new();
        text.push_str(r"#+attr_html: :file filename.ext");
        text.push_str("\n");
        text.push_str(r"#+caPtion[GIT]: org-rs");

        let mut cursor = Cursor::new(text.as_str(), 0);
        let maybe_affiliated = cursor.capturing_at(&*REGEX_AFFILIATED);

        assert!(maybe_affiliated.is_some());
    }

    #[test]
    fn collect_affiliated_small() {
        let mut text = String::new();
        text.push_str(r"#+caPtion[GIT]: org-rs");
        text.push_str("\n");
        text.push_str(r"#+attr_html: :file filename.ext");
        text.push_str("\n\n");
        {
            let p = Parser::new(text.as_str(), ParseGranularity::Object, DefaultEnvironment);
            let maybe_collected = p.collect_affiliated_keywords(text.len());
            assert_eq!(0, maybe_collected.0);
            assert!(maybe_collected.1.is_none());
        }
        text.pop();
        text.push_str("#+BEGIN_SRC");

        let p = Parser::new(text.as_str(), ParseGranularity::Object, DefaultEnvironment);
        let maybe_collected = p.collect_affiliated_keywords(text.len());
        assert_eq!(0, maybe_collected.0);
        assert!(maybe_collected.1.is_some());
        let collected = maybe_collected.1.unwrap();
        let mut test_attrs: HashMap<String, Vec<Cow<str>>> = HashMap::new();
        test_attrs.insert(
            "ATTR_HTML".to_string(),
            vec![Cow::from(":file filename.ext")],
        );
        assert_eq!(test_attrs, collected.attr);

        let mut test_caption: Vec<DualVal<StringOrObject>> = vec![];
        test_caption.push(DualVal {
            value: StringOrObject::Raw(Cow::from("org-rs")),
            secondary: Some(StringOrObject::Raw(Cow::from("GIT"))),
        });

        assert_eq!(test_caption, collected.caption);
    }
}
