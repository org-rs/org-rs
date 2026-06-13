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

//! Tests for the parser - adapted from Emacs org-element test suite
//!
//! This module contains tests for unimplemented or incomplete features
//! based on the Emacs test-org-element.el test suite.

mod common;
use common::*;

#[test]
fn planning_all_timestamps() {
    for (input, desc) in &[
        ("* TODO Task\nDEADLINE: <2023-12-31>\n", "deadline"),
        ("* TODO Task\nSCHEDULED: <2023-12-31>\n", "scheduled"),
        ("* TODO Task\nCLOSED: [2023-12-31]\n", "closed"),
        (
            "* TODO Task\nDEADLINE: <2023-12-31> SCHEDULED: <2023-12-30> CLOSED: [2023-12-29]\n",
            "all timestamps",
        ),
    ] {
        let count = get_type_count(input, SyntaxT::Planning, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 planning ({}), found {}", desc, count);
    }
}
