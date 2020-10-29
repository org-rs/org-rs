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
/// LaTeX Environments
///
/// Pattern for LaTeX environments is:
///
/// \begin{NAME} CONTENTS \end{NAME}
///
/// NAME is constituted of alpha-numeric or asterisk characters.
///
/// CONTENTS can contain anything but the “\end{NAME}” string.
use crate::data::SyntaxNode;
use crate::parser::Parser;
use regex::Regex;

// TODO wirte latex regexes
lazy_static! {


    /// Regexp matching the beginning of a LaTeX environment.
    /// The environment is captured by the first group.
    pub static ref REGEX_LATEX_BEGIN_ENVIRIONMENT: Regex = Regex::new(r"^[ \t]*\\begin{([A-Za-z0-9*]+)}").unwrap();
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

impl<'a> Parser<'a> {
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
        maybe_aff: Option<AffiliatedData>,
    ) -> SyntaxNode<'a> {
        unimplemented!()
    }
}
