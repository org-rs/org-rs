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

use crate::blocks::BlockName;
use crate::from_input::FromInput;

/// Classification of a `#+` directive line, extracted from the bytes
/// immediately following the `#` character.
///
/// Constructed via `HashDirective::from(s)` where `s` is the input slice
/// starting at the `+` character.  `From` is infallible: every input
/// classifies as exactly one variant.
pub enum HashDirective<'a> {
    Block(BlockName<'a>),
    BabelCall,
    DynamicBlock,
    Keyword,
    Unknown,
}

impl<'a> From<&'a str> for HashDirective<'a> {
    fn from(s: &'a str) -> Self {
        if let Ok(name) = BlockName::from_input(s) {
            return HashDirective::Block(name);
        }
        let bytes = s.as_bytes();
        if bytes
            .get(..6)
            .is_some_and(|b| b.eq_ignore_ascii_case(b"+CALL:"))
        {
            return HashDirective::BabelCall;
        }
        if bytes
            .get(..6)
            .is_some_and(|b| b.eq_ignore_ascii_case(b"+BEGIN"))
        {
            let is_dyn = match bytes.get(6) {
                Some(&b':') => matches!(bytes.get(7), Some(b' ' | b'\t')),
                Some(&b' ') | Some(&b'\t') => true,
                _ => false,
            };
            if is_dyn {
                return HashDirective::DynamicBlock;
            }
        }
        if bytes.first() == Some(&b'+') {
            let word_len = bytes[1..]
                .iter()
                .take_while(|&&b| b != b' ' && b != b'\t' && b != b'\n' && b != b':')
                .count();
            if word_len > 0 && bytes.get(1 + word_len) == Some(&b':') {
                return HashDirective::Keyword;
            }
        }
        HashDirective::Unknown
    }
}
