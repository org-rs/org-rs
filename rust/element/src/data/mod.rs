pub mod arena;
pub mod entity;
pub mod node;
pub mod objects;
pub mod syntax;
pub mod types;

pub use arena::{
    ChunkArena, ChunkNodes, Forest, NodeArena, NodeEntries, NodeIds, Nodes, NodesByType,
    ParseArena, ParseForest, PostOrderIds,
};
pub use bumpalo::collections::Vec as BumpVec;
pub use entity::EntityData;
pub use node::{SyntaxNode, SyntaxNodeBuilder};
pub use objects::*;
pub use syntax::Syntax;
pub use types::SyntaxT;

use std::num::NonZeroU64;

/// A packed node identifier combining chunk index (upper 32 bits) and
/// local index + 1 (lower 32 bits).  The +1 encoding ensures the value is
/// always non-zero, preserving the `NonZeroU64` niche for `Option<NodeId>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(NonZeroU64);

impl NodeId {
    /// Create a `NodeId` from a chunk index and a local node index.
    ///
    /// `local` is the 0-based position within the chunk's node vector.
    /// The stored representation adds 1 to `local` so the resulting
    /// `NonZeroU64` is never zero.
    #[inline]
    pub fn new(chunk: u32, local: u32) -> Self {
        let raw = ((chunk as u64) << 32) | (local as u64 + 1);
        NodeId(unsafe { NonZeroU64::new_unchecked(raw) })
    }

    /// Return the chunk index component.
    #[inline]
    pub fn chunk(self) -> u32 {
        (self.0.get() >> 32) as u32
    }

    /// Return the local node index within its chunk.
    #[inline]
    pub fn local(self) -> u32 {
        (self.0.get() as u32).wrapping_sub(1)
    }
}

pub use bumpalo::collections::Vec as BumpVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: usize,
    pub end: usize,
}

impl From<(usize, usize)> for Interval {
    #[inline]
    fn from((start, end): (usize, usize)) -> Self {
        Interval { start, end }
    }
}

impl From<Interval> for std::ops::Range<usize> {
    #[inline]
    fn from(iv: Interval) -> Self {
        iv.start..iv.end
    }
}

impl From<std::ops::Range<usize>> for Interval {
    #[inline]
    fn from(r: std::ops::Range<usize>) -> Self {
        Interval {
            start: r.start,
            end: r.end,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn timestamp_with_time_component() {
        let raw = "<2023-12-31 10:30>";
        let ts = TimestampData::new(raw).expect("Timestamp with time should parse");
        assert_eq!(ts.hour_start, Some(10));
        assert_eq!(ts.minute_start, Some(30));
    }

    #[test]
    fn can_contain() {
        let bold = SyntaxT::Bold;
        let br = SyntaxT::LineBreak;
        let verse = SyntaxT::VerseBlock;

        fn closure_test(that: SyntaxT, restriction: impl Fn(SyntaxT) -> bool) -> bool {
            restriction(that)
        }

        assert!(!bold.can_contain(SyntaxT::VerseBlock));
        assert!(bold.can_contain(SyntaxT::LineBreak));
        assert!(closure_test(br, |that| bold.can_contain(that)));
        assert!(!closure_test(verse, |that| bold.can_contain(that)));
    }

    #[test]
    fn nodes_iter_with_a_single_element_returns_that_element() {
        let bump = bumpalo::Bump::new();
        let mut arena = ChunkArena::new();
        let root = arena.alloc(SyntaxNode::create_root(&bump));
        let forest = Forest::from_chunks(vec![arena], root);
        assert_eq!(forest.nodes().count(), 1);
    }

    #[test]
    fn nodes_iter_with_several_children_return_all() {
        let bump = bumpalo::Bump::new();
        let mut arena = ChunkArena::new();
        let parent = arena.alloc(SyntaxNode::create_root(&bump));
        const NUM_CHILDREN: usize = 4;
        let children: Vec<NodeId> = (0..NUM_CHILDREN)
            .map(|_| arena.alloc(SyntaxNode::create_root(&bump)))
            .collect();
        arena.set_children(parent, BumpVec::from_iter_in(children.clone(), &bump));

        let forest = Forest::from_chunks(vec![arena], parent);
        let results: Vec<&SyntaxNode> = forest.nodes().collect();

        assert_eq!(results.len(), NUM_CHILDREN + 1);
        assert!(matches!(results[0].data, Syntax::OrgData));
        for (idx, _child) in children.iter().enumerate() {
            let result_node = results[idx + 1];
            assert_eq!(
                result_node.parent,
                Some(parent),
                "Parent mismatch at idx {}",
                idx
            );
            assert!(matches!(result_node.data, Syntax::OrgData));
        }
    }

    #[test]
    fn nodes_iter_with_several_layers_return_all() {
        let bump = bumpalo::Bump::new();
        let mut arena = ChunkArena::new();
        const LEVELS: usize = 4;
        let mut ids: Vec<NodeId> = Vec::new();
        for _ in 0..LEVELS {
            ids.push(arena.alloc(SyntaxNode::create_root(&bump)));
        }
        for i in 1..LEVELS {
            arena.set_children(ids[i - 1], BumpVec::from_iter_in([ids[i]], &bump));
        }

        let forest = Forest::from_chunks(vec![arena], ids[0]);
        let results: Vec<&SyntaxNode> = forest.nodes().collect();

        assert_eq!(results.len(), LEVELS);
        for node in results.iter() {
            assert!(matches!(node.data, Syntax::OrgData));
        }
    }
}
