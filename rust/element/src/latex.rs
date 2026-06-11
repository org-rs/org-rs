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

use crate::affiliated::ElementSpan;
use crate::cursor::CachedRegex;
use crate::data::{Interval, NodeId, Syntax, SyntaxNode};
use crate::parser::Parser;
use memchr::memrchr;
use regex::Regex;

lazy_static! {
    /// Regexp matching the beginning of a LaTeX environment.
    /// The environment is captured by the first group.
    pub static ref REGEX_LATEX_BEGIN_ENVIRIONMENT: CachedRegex =
        CachedRegex::new(Regex::new(r"^[ \t]*\\begin\{([A-Za-z0-9*]+)\}").unwrap());
}

#[derive(Debug)]
pub struct LatexEnvironmentData<'a> {
    pub begin: usize,
    pub end: usize,
    pub post_blank: usize,
    pub value: &'a str,
}

impl<'a, 'b, Environment: crate::environment::Environment> Parser<'a, 'b, Environment> {
    #[inline]
    pub fn latex_environment_parser(&mut self, element_span: ElementSpan<'a, 'b>) -> NodeId {
        let ElementSpan {
            span: Interval { start, end: limit },
            affiliated,
        } = element_span;
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
                        let line_start = memrchr(b'\n', &self.input.as_bytes()[..match_start])
                            .map_or(0, |p| p + 1);
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
                    return self
                        .arena
                        .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump));
                }

                let post_blank = if end_pos < limit {
                    let remaining = &self.input[end_pos..limit];
                    remaining.len() - remaining.trim_start().len()
                } else {
                    0
                }
                .min(2);

                let value = &self.input[start..end_pos];
                let env_data = LatexEnvironmentData {
                    begin: start,
                    end: end_pos,
                    post_blank,
                    value,
                };

                return self.arena.alloc(
                    SyntaxNode::new(
                        Syntax::LatexEnvironment(self.bump.alloc(env_data)),
                        (start, end_pos),
                        self.bump,
                    )
                    .post_blank(post_blank)
                    .affiliated(affiliated)
                    .build(),
                );
            }
        }

        self.arena
            .alloc(SyntaxNode::fallback(self.input, start, limit, self.bump))
    }
}
