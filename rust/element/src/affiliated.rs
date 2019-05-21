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

use crate::data::StringOrObject;
use crate::data::SyntaxT;
use crate::parser::Parser;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use std::string::ParseError;

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
              r"(?im)^[ \t]*{}|{}|{}|{}|{}[ \t]*[^\n\r]*$",
              r"#\+(?:(?:(?P<CAPTION>CAPTION)|(?P<RESULTS>RESULTS?))(?:\[(?P<SECONDARY>.*)\])?",   // DUAL
              r"(?P<HEADER>HEADERS?)",
              r"(?P<PLOT>PLOT)",
              r"(?P<NAME>(?:DATA|LABEL|NAME|RESNAME|(?:S(?:OURC|RCNAM)|TBLNAM)E))",
              r"(?P<ATTR>ATTR_[-_A-Za-z0-9]+)):")
       ).unwrap();
}

/// Since CAPTION is both DUAL and PARSED DualVal has to be able to store Strings or StringOrObject
#[derive(Default, Debug)]
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
pub struct AffiliatedData<'a> {
    /// DUAL, PARSED, MULTI
    caption: Vec<DualVal<StringOrObject<'a>>>,
    /// MULTI
    header: Vec<Cow<'a, str>>,

    /// No special capabilities
    name: Option<Cow<'a, str>>,

    /// No special capabilities
    plot: Option<Cow<'a, str>>,

    /// DUAL
    results: Option<DualVal<Cow<'a, str>>>,

    /// MULTI
    attr: HashMap<Cow<'a, str>, Vec<Cow<'a, str>>>,
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

impl<'a> Parser<'a> {
    /// Collect affiliated keywords from point down to LIMIT.
    ///
    /// Most elements can have affiliated keywords.  When looking for an
    /// element beginning, we want to move before them, as they belong to
    /// that element, and, in the meantime, collect information they give
    /// into appropriate properties.  Hence the following function.
    ///
    /// Return a list whose CAR is the position at the first of them and
    /// CDR a plist of keywords and values and move point to the
    /// beginning of the first line after them.
    ///
    /// As a special case, if element doesn't start at the beginning of
    /// the line (e.g., a paragraph starting an item), CAR is current
    /// position of point and CDR is nil."
    /// elisp `defun org-element--collect-affiliated-keywords (limit)`
    ///
    /// NB: it looks like this function parses objects ignoring parser granularity settings
    pub fn collect_affiliated_keywords(&self, limit: usize) -> (usize, AffiliatedData) {
        if !self.cursor.borrow().is_bol() {
            return (self.cursor.borrow().pos(), Default::default());
        }
        let origin = self.cursor.borrow().pos();
        let restrict = |that| SyntaxT::Keyword.can_contain(that);

        let mut result: AffiliatedData = Default::default();

        loop {
            let maybe_affiliated = self.cursor.borrow().capturing_at(&*REGEX_AFFILIATED);
            let current_pos = self.cursor.borrow().pos();
            if current_pos >= limit || maybe_affiliated.is_none() {
                break;
            }
            let captures = maybe_affiliated.expect("Captures are expected here");

            if captures.name("CAPTION").is_some() {
                // Check if secondary value is present
                // parse value to StringOrObject
                // parse secondary to StringOrObject
                // Save everything to DualVal
            } else if captures.name("RESULTS").is_some() {
                // dual

            } else if captures.name("NAME").is_some() {
                // no special capabilities

            } else if captures.name("PLOT").is_some() {
                // no special capabilities

            } else if captures.name("HEADER").is_some() {
                // multi

            } else if captures.name("ATTR").is_some() {
                // multi
            }
        }

        //       (while (and (< (point) limit) (looking-at org-element--affiliated-re))
        // 	(let* ((raw-kwd (upcase (match-string 1)))
        // 	       ;; Apply translation to RAW-KWD.  From there, KWD is
        // 	       ;; the official keyword.
        // 	       (kwd (or (cdr (assoc raw-kwd
        // 				    org-element-keyword-translation-alist))
        // 			raw-kwd))
        // 	       ;; Find main value for any keyword.
        // 	       (value
        // 		(save-match-data
        // 		  (org-trim
        // 		   (buffer-substring-no-properties
        // 		    (match-end 0) (line-end-position)))))
        // 	       ;; PARSEDP is non-nil when keyword should have its
        // 	       ;; value parsed.
        // 	       (parsedp (member kwd org-element-parsed-keywords))
        // 	       ;; If KWD is a dual keyword, find its secondary
        // 	       ;; value.  Maybe parse it.
        // 	       (dualp (member kwd org-element-dual-keywords))
        // 	       (dual-value
        // 		(and dualp
        // 		     (let ((sec (match-string-no-properties 2)))
        // 		       (if (or (not sec) (not parsedp)) sec
        // 			 (save-match-data
        // 			   (org-element--parse-objects
        // 			    (match-beginning 2) (match-end 2) nil restrict))))))
        // 	       ;; Attribute a property name to KWD.
        // 	       (kwd-sym (and kwd (intern (concat ":" (downcase kwd))))))
        // 	  ;; Now set final shape for VALUE.
        // 	  (when parsedp
        // 	    (setq value
        // 		  (org-element--parse-objects
        // 		   (match-end 0)
        // 		   (progn (end-of-line) (skip-chars-backward " \t") (point))
        // 		   nil restrict)))
        // 	  (when dualp
        // 	    (setq value (and (or value dual-value) (cons value dual-value))))
        // 	  (when (or (member kwd org-element-multiple-keywords)
        // 		    ;; Attributes can always appear on multiple lines.
        // 		    (string-match "^ATTR_" kwd))
        // 	    (setq value (cons value (plist-get output kwd-sym))))
        // 	  ;; Eventually store the new value in OUTPUT.
        // 	  (setq output (plist-put output kwd-sym value))
        // 	  ;; Move to next keyword.
        // 	  (forward-line)))
        //       ;; If affiliated keywords are orphaned: move back to first one.
        //       ;; They will be parsed as a paragraph.
        //       (when (looking-at "[ \t]*$") (goto-char origin) (setq output nil))
        //       ;; Return value.
        //       (cons origin output))))

        unimplemented!()
    }
}

mod test {
    use super::REGEX_AFFILIATED;
    use regex::Match;

    #[test]
    fn test_re() {
        let expected = r"(?im)^[ \t]*#\+(?:(?:(?P<CAPTION>CAPTION)|(?P<RESULTS>RESULTS?))(?:\[(?P<SECONDARY>.*)\])?|(?P<HEADER>HEADERS?)|(?P<PLOT>PLOT)|(?P<NAME>(?:DATA|LABEL|NAME|RESNAME|(?:S(?:OURC|RCNAM)|TBLNAM)E))|(?P<ATTR>ATTR_[-_A-Za-z0-9]+)):[ \t]*[^\n\r]*$";
        assert_eq!(expected, REGEX_AFFILIATED.as_str());
    }

    #[test]
    fn affiliated_re() {
        let dual_full = r"#+caPtion[GIT]: org-rs";

        let mut cap = REGEX_AFFILIATED.captures(dual_full).unwrap();
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

}
