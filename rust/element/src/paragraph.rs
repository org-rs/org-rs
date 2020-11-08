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
use std::cell::Cell;
use std::cell::RefCell;

use crate::affiliated::AffiliatedData;
use crate::cursor::LinesMetric;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;

use crate::drawer::REGEX_DRAWER;
use crate::latex::FMTSTR_LATEX_END_ENVIRONMENT;
use crate::latex::REGEX_LATEX_BEGIN_ENVIRIONMENT;

use regex::Regex;

lazy_static! {
    static ref REGEX_PARAGRAPH_SEPARATE: Regex = Regex::new({
        let static_content = concat!(r"^\(?:",
                                     // Headlines, inlinetasks.
                                     r"\*+ ",
                                     r"\|",
                                     // Footnote definitions.
                                     r"\[fn:[-_[:word:]]+\]",
                                     "\\|",
                                     // Diary sexps.
                                     "%%(",
                                     r"\|",
                                     r"[ \t]*\(?:",
                                     // Empty lines.
                                     "$",
                                     r"\|",
                                     // Tables (any type).
                                     "|",
                                     r"\|",
                                     r"\+\(?:-+\+\)+[ \t]*$",
                                     r"\|",
                                     // Comments, keyword-like or block-like constructs.
                                     // Blocks and keywords with dual values need to be
                                     // double-checked.
                                     r"#\(?: \|$\|\+\(?:",
                                     r"BEGIN_\S-+",
                                     r"\|",
                                     r"\S-+\(?:\[.*\]\)?:[ \t]*\)\)",
                                     r"\|",
                                     // Drawers (any type) and fixed-width areas.  Drawers
                                     // need to be double-checked.
                                     r":\(?: \|$\|[-_[:word:]]+:[ \t]*$\)",
                                     r"\|",
                                     // Horizontal rules.
                                     r"-\{5,\}[ \t]*$",
                                     r"\|",
                                     // LaTeX environments.
                                     r"\\begin{\([A-Za-z0-9*]+\)}",
                                     r"\|",
                                     // Clock lines.
                                     "CLOCK:",
                                     r"\|"
        ).to_owned();
        // Lists.
        // TODO: How to handle these config parameters
        let term = match /* org-plain-list-ordered-item-terminator */ 0 as char {
            ')' => ")",
            '.' => r"\.",
            _ => "[.)]",
        };
        let alpha = if /* org-list-allow-alphabtical */ true{
            r"\|[A-Za-z]"
        } else {
            ""
        };
        &(static_content +  "\\(?:[-+*]\\|\\(?:[0-9]+" + alpha + "\\)" + term + "\\)"
            + "\\(?:[ \t]\\|$\\)"
            + "\\)\\)")
    }).unwrap();
}

/// List of affiliated keywords which can have a secondary value.
///
/// In Org syntax, they can be written with optional square brackets
/// before the colons.  For example, RESULTS keyword can be
/// associated to a hash value with the following:
///
///   #+RESULTS[hash-string]: some-source
///
/// This list is checked after translations have been applied.  See
/// ‘org-element-keyword-translation-alist’.
const DUAL_KEYWORDS: &[&'static str] = &["CAPTION", "RESULTS"];

/// Parse a paragraph.
///
/// LIMIT bounds the search.  AFFILIATED is a list of which CAR is
/// the buffer position at the beginning of the first affiliated
/// keyword and CDR is a plist of affiliated keywords along with
/// their value.
///
/// Return a list whose CAR is `paragraph' and CDR is a plist
/// containing `:begin', `:end', `:contents-begin' and
/// `:contents-end', `:post-blank' and `:post-affiliated' keywords.
///
/// Assume point is at the beginning of the paragraph."
/// (defun org-element-paragraph-parser (limit affiliated)
impl<'a> Parser<'a> {
    // TODO implement paragraph_parser
    pub fn paragraph_parser(
        &self,
        limit: usize,
        start: usize,
        maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let before_blank = {
            // Why is this variable in the origional elisp?
            let case_fold_search = true;
            self.cursor.borrow_mut().goto_line_begin();
            // A matching `org-element-paragraph-separate' is not
            // necessarily the end of the paragraph.  In particular,
            // drawers, blocks or LaTeX environments opening lines
            // must be closed.  Moreover keywords with a secondary
            // value must belong to "dual keywords".
            while !{
                if !(self
                    .cursor
                    .borrow_mut()
                    .re_search_forward(&REGEX_PARAGRAPH_SEPARATE, Some(limit))
                    .is_some()
                    && {
                        self.cursor.borrow_mut().goto_line_begin();
                        true
                    })
                {
                    true
                } else if looking_at!(REGEX_DRAWER, self).is_some() {
                    let pos = self.cursor.borrow().pos();
                    lazy_static! {
                        static ref REGEX: Regex = Regex::new("^[ \t]*:END:[ \t]*$").unwrap();
                    }
                    // This line ignores the t passed in the third parameter (NOERROR) to re-search-forward, but since we are about to end our excursion, there is no overall difference in behavior
                    let res = self
                        .cursor
                        .borrow_mut()
                        .re_search_forward(&REGEX, Some(limit))
                        .is_some();
                    self.cursor.borrow_mut().set(pos);
                    res
                } else if let Some(caps) = {
                    lazy_static! {
                        static ref REGEX: Regex =
                            Regex::new("[ \t]*#\\+BEGIN_\\(\\S-+\\)").unwrap();
                    }
                    capturing_at!(REGEX, self)
                } {
                    let pos = self.cursor.borrow().pos();
                    // This ignores the t passed in the third parameter (NOERROR) to
                    // re-search-forward, but since we are about to end our excursion, there is
                    // no overall difference in behavior
                    let res = self
                        .cursor
                        .borrow_mut()
                        .re_search_forward(
                            &Regex::new(&format!(
                                "^[ \t]*#\\+END_{}[ \t]*$",
                                regex::escape(caps.get(1).unwrap().as_str())
                            ))
                            .unwrap(),
                            Some(limit),
                        )
                        .is_some();
                    self.cursor.borrow_mut().set(pos);
                    res
                } else if let Some(caps) = capturing_at!(REGEX_LATEX_BEGIN_ENVIRIONMENT, self) {
                    let pos = self.cursor.borrow().pos();
                    // This ignores the t passed in the third parameter (NOERROR) to
                    // re-search-forward, but since we are about to end our excursion, there is
                    // no overall difference in behavior
                    let res = self
                        .cursor
                        .borrow_mut()
                        .re_search_forward(
                            {
                                let mut fmtstr = FMTSTR_LATEX_END_ENVIRONMENT.to_owned();
                                let start = fmtstr.find("%s").unwrap();
                                fmtstr.replace_range(
                                    start..(start + 2),
                                    &regex::escape(caps.get(1).unwrap().as_str()),
                                );

                                &Regex::new(&fmtstr).unwrap()
                            },
                            Some(limit),
                        )
                        .is_some();
                    self.cursor.borrow_mut().set(pos);
                    res
                } else if let Some(caps) = {
                    lazy_static! {
                        static ref REGEX: Regex =
                            Regex::new("[ \t]*#\\+\\(\\S-+\\)\\[.*\\]:").unwrap();
                    }
                    capturing_at!(REGEX, self)
                } {
                    DUAL_KEYWORDS
                        .iter()
                        .find(|keyword|
                              // This is not an ideal way to do a case-insensitive search
                              keyword.to_uppercase() == caps.get(1).unwrap().as_str().to_uppercase())
                        .is_some()
                } else {
                    // Everything else is unambiguous.
                    true
                }
            } {
                self.cursor
                    .borrow_mut()
                    .set(self.cursor.borrow_mut().line_end_position(None));
            }
            if self.cursor.borrow().pos() == limit {
                limit
            } else {
                self.cursor.borrow_mut().goto_line_begin()
            }
        };
        let contents_end = {
            let pos = self.cursor.borrow().pos();
            self.cursor
                .borrow_mut()
                .skip_chars_backward(" \r\t\n", Some(start));
            self.cursor.borrow_mut().goto_next_line();
            self.cursor.borrow_mut().goto_next_line();
            let res = self
                .cursor
                .borrow_mut()
                .at_or_next::<LinesMetric>()
                .unwrap();
            self.cursor.borrow_mut().set(pos);
            res
        };
        let end = {
            self.cursor
                .borrow_mut()
                .skip_chars_forward(" \r\t\n", Some(limit));
            if self.cursor.borrow().pos() == self.input.len() {
                self.cursor.borrow().pos()
            } else {
                self.cursor
                    .borrow_mut()
                    .at_or_prev::<LinesMetric>()
                    .unwrap()
            }
        };
        SyntaxNode {
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
            data: Syntax::Paragraph,
            location: Interval { start, end },
            content_location: Some(Interval {
                start,
                end: contents_end,
            }),
            post_blank: self
                .cursor
                .borrow_mut()
                .count_boundries::<LinesMetric>(before_blank, end),
            // TODO: Figure out what :post-affiliated maps to in the rust code.
            // TODO: Unstubbify and add the affiliated data.
            affiliated: Some(()),
        }
    }
    // TODO implement section_parser
    pub fn section_parser(&self, limit: usize) -> SyntaxNode<'a> {
        unimplemented!()
    }
}


#[cfg(test)]
mod tests {
    //! Tests for paragraph parser.
    //! Translated from https://code.orgmode.org/bzg/org-mode/src/master/testing/lisp/test-org-element.el#L1954
// (ert-deftest test-org-element/paragraph-parser ()
//   "Test `paragraph' parser."
//   ;; Standard test.
//   (should
//    (org-test-with-temp-text "Paragraph"
//      (org-element-map (org-element-parse-buffer) 'paragraph 'identity nil t)))
//   ;; Property find end of a paragraph stuck to another element.
//   (should
//    (eq ?#
//        (org-test-with-temp-text "Paragraph\n# Comment"
// 	 (org-element-map (org-element-parse-buffer) 'paragraph
// 	   (lambda (p) (char-after (org-element-property :end p)))
// 	   nil t))))
//   ;; Include ill-formed Keywords.
//   (should
//    (org-test-with-temp-text "#+wrong_keyword something"
//      (org-element-map (org-element-parse-buffer) 'paragraph 'identity)))
//   ;; Include incomplete-drawers.
//   (should
//    (org-test-with-temp-text ":TEST:\nParagraph"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (= (point-max) (org-element-property :end elem))))))
//   ;; Include incomplete blocks.
//   (should
//    (org-test-with-temp-text "#+BEGIN_CENTER\nParagraph"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (= (point-max) (org-element-property :end elem))))))
//   ;; Include incomplete dynamic blocks.
//   (should
//    (org-test-with-temp-text "#+BEGIN: \nParagraph"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (= (point-max) (org-element-property :end elem))))))
//   ;; Include incomplete latex environments.
//   (should
//    (org-test-with-temp-text "\begin{equation}\nParagraph"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (= (point-max) (org-element-property :end elem))))))
//   (should
//    (org-test-with-temp-text "Paragraph\n\begin{equation}"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (= (point-max) (org-element-property :end elem))))))
//   ;; Stop at affiliated keywords.
//   (should
//    (org-test-with-temp-text "Paragraph\n#+NAME: test\n| table |"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (not (org-element-property :name elem))
// 	    (= (org-element-property :end elem) (line-beginning-position 2))))))
//   (should
//    (org-test-with-temp-text
//        "Paragraph\n#+CAPTION[with short caption]: test\n| table |"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (not (org-element-property :name elem))
// 	    (= (org-element-property :end elem) (line-beginning-position 2))))))
//   ;; Do not steal affiliated keywords from container.
//   (should
//    (org-test-with-temp-text "#+ATTR_LATEX: test\n- item<point> 1"
//      (let ((elem (org-element-at-point)))
//        (and (eq (org-element-type elem) 'paragraph)
// 	    (not (org-element-property :attr_latex elem))
// 	    (/= (org-element-property :begin elem) 1)))))
//   ;; Handle non-empty blank line at the end of buffer.
//   (should
//    (org-test-with-temp-text "#+BEGIN_CENTER\nC\n#+END_CENTER\n  "
//      (= (org-element-property :end (org-element-at-point)) (point-max)))))
//
}
