#![allow(dead_code)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;

extern crate regex;
extern crate strum;

mod cursor;
mod data;
mod headline;
mod list;
mod paragraph;
mod parser;
mod planning;
mod table;
