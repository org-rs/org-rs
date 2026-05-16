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

use crate::affiliated::AffiliatedData;
use crate::data::{Interval, Syntax, SyntaxNode};
use crate::parser::Parser;
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;

// TODO wirte latex regexes
lazy_static! {


    /// Regexp matching the beginning of a LaTeX environment.
    /// The environment is captured by the first group.
    pub static ref REGEX_LATEX_BEGIN_ENVIRIONMENT: Regex = Regex::new(r"^[ \t]*\\begin\{([A-Za-z0-9*]+)\}").unwrap();
}

/// Format string matching the ending of a LaTeX environment
/// Unfortunately because of the way original elisp parser is written this
/// regex can't be made static as it should match the opening part
///
/// In ideal world this should be replaced by a proper parser
pub static FMTSTR_LATEX_END_ENVIRONMENT: &str = r"\\end{%s}[ \t]*$";

#[derive(Debug)]
pub struct LatexEnvironmentData<'a> {
    /// Buffer position at first affiliated keyword or
    /// at the beginning of the first line of environment (integer).
    begin: usize,

    /// Buffer position at the first non_blank line
    /// after last line of the environment, or buffer's end (integer).
    end: usize,

    /// Number of blank lines between last environment's
    /// line and next non_blank line or buffer's end (integer).
    post_blank: usize,

    ///LaTeX code (string).
    value: &'a str,
}

#[derive(Debug)]
pub struct LatexFragmentData<'a> {
    ///LaTeX code (string).
    value: &'a str,
}

impl<'a, Environment: crate::environment::Environment> Parser<'a, Environment> {
    // TODO implement latext_environment_parser
    /// Parse a LaTeX environment.
    /// LIMIT bounds the search.  AFFILIATED is a list of which CAR is
    /// the buffer position at the beginning of the first affiliated
    /// keyword and CDR is a plist of affiliated keywords along with
    /// their value.
    ///
    /// Return a list whose CAR is `latex-environment' and CDR is a plist
    /// containing `:begin', `:end', `:value', `:post-blank' and
    /// `:post-affiliated' keywords.
    ///
    /// Assume point is at the beginning of the latex environment."
    pub fn latex_environment_parser(
        &self,
        limit: usize,
        start: usize,
        _maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        let input_slice = &self.input[start..limit];

        if let Some(caps) = REGEX_LATEX_BEGIN_ENVIRIONMENT.captures(input_slice) {
            if let Some(env_name) = caps.get(1) {
                let env_name_str = env_name.as_str();
                let begin_line_end = start + caps.get(0).map_or(caps.len(), |m| m.end());

                let end_marker = format!("\\end{{{}}}", env_name_str);
                let mut search_pos = begin_line_end;
                let mut end_pos = limit;
                let mut found = false;

                while search_pos < limit {
                    if let Some(idx) = self.input[search_pos..limit].find(&end_marker) {
                        let match_start = search_pos + idx;
                        let match_end = match_start + end_marker.len();
                        let line_start = self.input[..match_start].rfind('\n').map_or(0, |p| p + 1);
                        let line = &self.input[line_start..match_start];

                        if line.chars().all(|c| c == ' ' || c == '\t' || c == '\n') {
                            end_pos = if match_end < limit
                                && self.input.as_bytes().get(match_end) == Some(&b'\n')
                            {
                                match_end + 1
                            } else {
                                match_end
                            };
                            found = true;
                            break;
                        }
                        search_pos = match_end;
                    } else {
                        break;
                    }
                }

                if !found {
                    return SyntaxNode::fallback(self.input, start, limit);
                }

                let post_blank = if end_pos < limit {
                    let remaining = &self.input[end_pos..limit];
                    let trimmed = remaining.trim_start();
                    (remaining.len() - trimmed.len()).min(2)
                } else {
                    0
                };

                let value = &self.input[start..end_pos];
                let env_data = LatexEnvironmentData {
                    begin: start,
                    end: end_pos,
                    post_blank,
                    value,
                };

                return SyntaxNode {
                    parent: RefCell::new(None),
                    children: RefCell::new(vec![]),
                    data: Syntax::LatexEnvironment(Box::new(env_data)),
                    location: Interval {
                        start,
                        end: end_pos,
                    },
                    content_location: None,
                    post_blank,
                    affiliated: None,
                };
            }
        }

        SyntaxNode::fallback(self.input, start, limit)
    }
}
