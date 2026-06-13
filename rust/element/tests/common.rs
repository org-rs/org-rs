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

#![allow(unused_imports)]
pub use bumpalo::Bump;
pub use org_element::data::{NodeArena, NodeId, Syntax, SyntaxT};
pub use org_element::environment::DefaultEnvironment;
pub use org_element::parser::{ParseGranularity, Parser};

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

pub fn get_type_count(input: &str, typ: SyntaxT, granularity: ParseGranularity) -> usize {
    let bump = Bump::new();
    let mut parser = Parser::new(input, granularity, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    count_type(&arena, root, typ)
}
