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

//! Convenience helpers for querying parse trees.
//!
//! These utilities are primarily used by the integration test suite, but are
//! also useful for examples and ad-hoc tree inspection. They are re-exported,
//! together with the types they need, so a single glob import is enough:
//!
//! ```rust,ignore
//! use org_element::testutils::*;
//! ```

pub use crate::data::{NodeArena, NodeId, Syntax, SyntaxT};
pub use crate::environment::DefaultEnvironment;
pub use crate::parser::{ParseGranularity, Parser};
pub use bumpalo::Bump;

/// Recursively counts the nodes of type `typ` in the subtree rooted at `id`.
pub fn count_type(arena: &NodeArena, id: NodeId, typ: SyntaxT) -> usize {
    let mut count = 0;
    if SyntaxT::from(&arena[id].data) == typ {
        count += 1;
    }
    for &child in &arena[id].children {
        count += count_type(arena, child, typ);
    }
    count
}

/// Parses `input` at the given `granularity` and counts the nodes of type `typ`.
pub fn get_type_count(input: &str, typ: SyntaxT, granularity: ParseGranularity) -> usize {
    let bump = Bump::new();
    let mut parser = Parser::new(input, granularity, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    count_type(&arena, root, typ)
}
