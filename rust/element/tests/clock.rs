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

use org_element::testutils::*;

#[test]
fn clock_variants() {
    for (input, desc) in &[
        ("CLOCK: [2023-10-13 Fri 14:40]\n", "running"),
        (
            "CLOCK: [2023-10-13 Fri 14:40]--[2023-10-13 Fri 14:51] => 0:11\n",
            "closed",
        ),
        ("Clock: [2023-10-13 Fri 14:40]\n", "case insensitive"),
        ("CLOCK: => 0:11\n", "duration only"),
    ] {
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock ({}), found {}", desc, count);
    }
}

#[test]
fn must_be_bol() {
    for (input, desc) in &[
        ("   CLOCK: [2023-10-13 Fri 14:40]\n", "leading spaces"),
        ("\tCLOCK: [2023-10-13 Fri 14:40]\n", "leading tab"),
    ] {
        let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
        assert_eq!(count, 1, "Expected 1 clock with {}, found {}", desc, count);
    }
}

#[test]
fn invalid_date() {
    let input = "CLOCK: [2023-02-29 Wed 14:40]\n";
    let count = get_type_count(input, SyntaxT::Clock, ParseGranularity::Element);
    assert_eq!(
        count, 0,
        "Expected 0 clock (Feb 29 in non-leap year), found {}",
        count
    );
}
