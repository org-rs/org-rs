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

#![warn(clippy::all)]
// This should be eventually turned off, but for now this helps reduce the noise
#![allow(dead_code)]
#![allow(warnings)]
#[macro_use]
extern crate lazy_static;
extern crate memchr;
extern crate regex;

#[macro_use]
pub mod parser;
mod affiliated;
mod babel;
pub mod blocks;
pub mod cursor;
pub mod data;
mod drawer;
pub mod environment;
mod fixed_width;
pub mod headline;
pub mod keyword;
mod latex;
mod list;
pub mod markup;
mod paragraph;
mod planning;
mod table;

/// Commonly needed types, re-exported for convenience.
///
/// Most element-parser modules need the same small set of imports.  Rather
/// than repeating long `use crate::data::{…}` / `use crate::parser::{…}`
/// lines everywhere, a single `use crate::prelude::*;` is enough.
pub mod prelude {
    pub use crate::affiliated::AffiliatedData;
    pub use crate::data::{Interval, Syntax, SyntaxNode, SyntaxT};
    pub use crate::parser::{ParseGranularity, Parser};
}

#[cfg(test)]
mod parser_tests;
