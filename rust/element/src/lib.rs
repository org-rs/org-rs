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
#![allow(dead_code)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;

extern crate memchr;
extern crate regex;
extern crate strum;

mod affiliated;
mod cursor;
mod data;
mod headline;
mod list;
mod paragraph;
mod parser;
mod planning;
mod table;
