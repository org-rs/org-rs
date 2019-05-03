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
//! With the exception of inlinetasks, items, planning, clocks,
//! node properties and table rows, every other element type
//! can be assigned attributes.  
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
//! “CAPTION”, “AUTHOR”, “DATE” and “TITLE” keywords can contain objects
//! in their value and their optional value, if applicable.

use crate::parser::Parser;
use regex::Regex;

lazy_static! {

   /// FIXME Regexp matching any affiliated keyword
   /// Keyword name is put in match group 1.  Moreover, if keyword
   /// belongs to `org-element-dual-keywords', put the dual value in
   /// match group 2.
   ///
   ///
    //   (format "[ \t]*#\\+\\(?:%s\\):[ \t]*"
    // 	  (concat
    // 	   ;; Dual affiliated keywords.
    // 	   (format "\\(?1:%s\\)\\(?:\\[\\(.*\\)\\]\\)?"
    // 		   (regexp-opt org-element-dual-keywords))
    // 	   "\\|"
    // 	   ;; Regular affiliated keywords.
    // 	   (format "\\(?1:%s\\)"
    // 		   (regexp-opt
    // 		    (cl-remove-if
    // 		     (lambda (k) (member k org-element-dual-keywords))
    // 		     org-element-affiliated-keywords)))
    // 	   "\\|"
    // 	   ;; Export attributes.
    // 	   "\\(?1:ATTR_[-_A-Za-z0-9]+\\)"))
   pub static ref REGEX_AFFILIATED: Regex = Regex::new(r"").unwrap();

// [ \t]*#\+(?:(?1:(?:CAPTION|RESULTS))(?:[(.*)])?|(?1:(?:DATA|HEADERS?|LABEL|NAME|PLOT|RES(?:NAME|ULT)|(?:S(?:OURC|RCNAM)|TBLNAM)E))|(?1:ATTR_[-_A-Za-z0-9]+)):[ \t]*

}

pub enum Affiliated {
    Regular,
    Dual,
    ExportAttribute,
}

/// List of affiliated keywords as strings.
/// By default, all keywords setting attributes (e.g., \"ATTR_LATEX\")
/// are affiliated keywords and need not to be in this list.")
/// elisp: `defconst org-element-affiliated-keywords`
pub enum Keywords {
    CAPTION,
    DATA,
    HEADER,
    HEADERS,
    LABEL,
    NAME,
    PLOT,
    RESNAME,
    RESULT,
    RESULTS,
    SOURCE,
    SRCNAME,
    TBLNAME,
}

impl Keywords {
    /// Translates old keyword value into a new one
    /// elisp: `defconst org-element-keyword-translation-alist`
    fn translate(&mut self) {
        use std::mem;
        use Keywords::*;
        match self {
            DATA | LABEL | RESNAME | SOURCE | SRCNAME | TBLNAME => drop(mem::replace(self, NAME)),
            RESULT => drop(mem::replace(self, RESULTS)),
            HEADERS => drop(mem::replace(self, HEADER)),
            _ => (),
        };
    }

    /// List of affiliated keywords that can occur more than once in an element.
    ///
    /// Their value will be consed into a list of strings, which will be
    /// returned as the value of the property.
    ///
    /// This list is checked after translations have been applied.  See
    /// `org-element-keyword-translation-alist'.
    ///
    /// By default, all keywords setting attributes (e.g., \"ATTR_LATEX\")
    /// allow multiple occurrences and need not to be in this list.
    /// elisp: `defconst org-element-multiple-keywords '("CAPTION" "HEADER")`
    fn is_multiple_allowed(&self) -> bool {
        use Keywords::*;
        match self {
            CAPTION | HEADER => true,
            _ => false,
        }
    }

    /// List of affiliated keywords whose value can be parsed.
    ///
    /// Their value will be stored as a secondary string: a list of
    /// strings and objects.
    ///
    /// elisp: `defconst org-element-parsed-keywords
    /// This list is checked after translations have been applied.
    /// See `traslate`, `org-element-keyword-translation-alist`
    fn can_contain_objects(&self) -> bool {
        use Keywords::*;
        match &self {
            CAPTION => true,
            _ => false,
        }
    }
}

//
//
//
// (defconst org-element--parsed-properties-alist
//   (mapcar (lambda (k) (cons k (intern (concat ":" (downcase k)))))
// 	  org-element-parsed-keywords)
//   "Alist of parsed keywords and associated properties.
// This is generated from `org-element-parsed-keywords', which
// see.")
//
// (defconst org-element-dual-keywords '("CAPTION" "RESULTS")
//   "List of affiliated keywords which can have a secondary value.
//
// In Org syntax, they can be written with optional square brackets
// before the colons.  For example, RESULTS keyword can be
// associated to a hash value with the following:
//
//   #+RESULTS[hash-string]: some-source
//
// This list is checked after translations have been applied.  See
// `org-element-keyword-translation-alist'.")
//
//
//
// Don't modify it, set `org-element-affiliated-keywords' instead.")

impl<'a> Parser<'a> {
    // TODO implement collect_affiliated_keywords
    pub fn collect_affiliated_keywords(&self) -> Affiliated {
        unimplemented!()
    }
}

//
// (defun org-element--collect-affiliated-keywords (limit)
//   "Collect affiliated keywords from point down to LIMIT.
//
// ;; Most elements can have affiliated keywords.  When looking for an
// ;; element beginning, we want to move before them, as they belong to
// ;; that element, and, in the meantime, collect information they give
// ;; into appropriate properties.  Hence the following function.

// Return a list whose CAR is the position at the first of them and
// CDR a plist of keywords and values and move point to the
// beginning of the first line after them.
//
// As a special case, if element doesn't start at the beginning of
// the line (e.g., a paragraph starting an item), CAR is current
// position of point and CDR is nil."
//   (if (not (bolp)) (list (point))
//     (let ((case-fold-search t)
// 	  (origin (point))
// 	  ;; RESTRICT is the list of objects allowed in parsed
// 	  ;; keywords value.
// 	  (restrict (org-element-restriction 'keyword))
// 	  output)
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
