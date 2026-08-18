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

//! Parse an `.org` file and print its element/object tree with byte
//! offsets and a short snippet per node.  Handy for diagnosing corpus
//! discrepancies against the Emacs oracle:
//!
//! ```sh
//! cargo run --example debug_dump path/to/file.org
//! ```

use bumpalo::Bump;
use org_element::data::{NodeArena, NodeId, SyntaxT};
use org_element::environment::DefaultEnvironment;
use org_element::parser::{ParseGranularity, Parser};

fn dump(arena: &NodeArena, id: NodeId, depth: usize, input: &str) {
    let node = &arena[id];
    let typ = SyntaxT::from(&node.data);
    let loc = node.location;
    let snippet: String = input
        .get(loc.start..loc.end.min(input.len()))
        .unwrap_or("")
        .chars()
        .take(40)
        .collect();
    println!(
        "{}{:?} [{}..{}] {:?}",
        "  ".repeat(depth),
        typ,
        loc.start,
        loc.end,
        snippet
    );
    for &c in &node.children {
        dump(arena, c, depth + 1, input);
    }
}

fn main() {
    let input = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let bump = Bump::new();
    let mut parser = Parser::new(&input, ParseGranularity::Object, DefaultEnvironment, &bump);
    let (arena, root) = parser.parse_buffer();
    dump(&arena, root, 0, &input);
}
