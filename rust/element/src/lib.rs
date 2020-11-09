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
// This should be eventually turned off, but for now this helps reduce the noice
#![allow(dead_code)]
#![allow(warnings)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;

extern crate memchr;
extern crate regex;
extern crate strum;

#[macro_use]
mod parser;
mod affiliated;
mod babel;
mod blocks;
mod cursor;
mod data;
mod drawer;
mod environment;
mod fixed_width;
mod headline;
mod keyword;
mod latex;
mod list;
mod markup;
mod paragraph;
mod planning;
mod table;
