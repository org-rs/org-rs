use rayon::prelude::*;

use super::{collect_radio_targets, ParseGranularity, Parser, ParserMode};
use crate::cursor::Cursor;
use crate::data::{BumpVec, ChunkArena, Forest, Interval, NodeId, Syntax, SyntaxNode};
use crate::environment;

/// Thread-safe wrapper around a per-thread parse result.
///
/// # Safety
///
/// Each `SendChunk` is created and consumed on a single thread.
/// `ChunkArena` is not automatically `Send` because `SyntaxNode`
/// contains `BumpVec` (which stores `NonNull` pointers), but the
/// arena is never accessed concurrently – it is assembled on one
/// worker and moved to the collecting thread.
struct SendChunk<'a>(ChunkArena<'a, 'a>, Vec<NodeId>);

// SAFETY: per comment above.
unsafe impl<'a> Send for SendChunk<'a> {}

/// Greedily group headline positions into byte-balanced spans.
///
/// Produces `N ≈ cores × chunk_multiplier` groups (minimum 8).  Each
/// group spans one or more consecutive headlines whose combined byte size
/// is roughly `total_bytes / N`.  The preamble (text before the first
/// headline) gets its own span; each remaining span starts at a headline
/// boundary.
fn balance_spans(input: &str, starts: &[usize], chunk_multiplier: usize) -> Vec<(usize, usize)> {
    let num_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let target_count = (num_cores * chunk_multiplier).max(8);
    let total = input.len();
    let target_bytes = (total / target_count).max(1);

    if starts.is_empty() {
        return vec![(0, total)];
    }

    let mut spans: Vec<(usize, usize)> = Vec::new();

    // Preamble (text before the first headline).
    if starts[0] > 0 {
        spans.push((0, starts[0]));
    }

    // Greedy group: include as many consecutive headline-spans as fit
    // within `target_bytes` per group.  A group always covers a
    // contiguous region from its first headline's start to wherever the
    // next group would begin (which is the first headline not included).
    let mut i = 0;
    while i < starts.len() {
        let group_start = starts[i];
        // Advance j while the span from group_start to starts[j] stays
        // within target_bytes.
        let mut j = i + 1;
        while j < starts.len() && (starts[j] - group_start) < target_bytes {
            j += 1;
        }
        let group_end = if j < starts.len() { starts[j] } else { total };
        spans.push((group_start, group_end));
        i = j;
    }

    spans
}

/// Parse one chunk, returning its arena and top-level element ids.
///
/// The parser is bounded by `span.end` so that `find_headline_end` (and
/// therefore headline content boundaries) never extend past the chunk.
fn parse_chunk_with_id<'a, E: environment::Environment + Clone>(
    input: &'a str,
    chunk_id: u32,
    span: (usize, usize),
    granularity: ParseGranularity,
    environment: &E,
    tab_width: u8,
    radio_targets: Vec<&'a str>,
) -> SendChunk<'a> {
    let bump: Box<bumpalo::Bump> = Box::new(bumpalo::Bump::new());

    let (arena, top_elements) = {
        let mut parser = Parser {
            cursor: Cursor::new(input, span.0),
            input,
            granularity,
            environment: environment.clone(),
            arena: ChunkArena::with_id(chunk_id, span.0),
            bump: &*bump,
            object_region_start: 0,
            tab_width,
            item_indent_ctx: None,
            radio_targets,
        };
        let children = parser.parse_elements(span, ParserMode::FirstSection, None);
        let top: Vec<NodeId> = children.iter().copied().collect();
        let arena = std::mem::take(&mut parser.arena);
        (arena, top)
    };

    // SAFETY: arena was created with &*bump (lifetime '_). Transmuting
    // to 'a is sound because bump is boxed and stored alongside the arena.
    let mut arena: ChunkArena<'a, 'a> = unsafe { std::mem::transmute(arena) };
    arena._bumps.push(bump);

    SendChunk(arena, top_elements)
}

/// Spine pass: create the root chunk and link parent/child relationships
/// across data chunks so that the forest structurally matches a serial
/// parse.  Each data chunk's headline-level top elements are re-parented
/// under the correct ancestor via a simple headline-level stack.
fn spine_pass<'a>(
    input: &'a str,
    data_results: Vec<(ChunkArena<'a, 'a>, Vec<NodeId>)>,
) -> Forest<'a, 'a> {
    // Root chunk (id = 0).
    let root_bump: Box<bumpalo::Bump> = Box::new(bumpalo::Bump::new());
    let mut root_arena = ChunkArena::with_id(0, 0);
    root_arena._bumps.push(root_bump);
    let root_ref: &bumpalo::Bump = &*root_arena._bumps[0];
    let mut root_children: BumpVec<'_, NodeId> = BumpVec::new_in(root_ref);

    // Determine parent for every data-chunk top element.
    // `links` records (child, parent) pairs that cross chunk boundaries.
    struct Link {
        child: NodeId,
        parent: NodeId,
    }
    let mut links: Vec<Link> = Vec::new();
    // Stack: (headline-id, level)
    let mut stack: Vec<(NodeId, usize)> = Vec::new();

    for (arena, top_ids) in &data_results {
        for &id in top_ids {
            let node = &arena[id];
            let level = match &node.data {
                Syntax::Headline(data) => data.level,
                _ => 0,
            };

            // Pop stack until we find a headline strictly shallower
            // (lower level number) than the current element.
            while let Some(&(_, lvl)) = stack.last() {
                if lvl >= level {
                    stack.pop();
                } else {
                    break;
                }
            }

            let parent = if level > 0 {
                stack.last().map(|&(pid, _)| pid)
            } else {
                None
            };

            match parent {
                Some(pid) => links.push(Link {
                    child: id,
                    parent: pid,
                }),
                None => {
                    root_children.push(id);
                }
            }

            if level > 0 {
                stack.push((id, level));
            }
        }
    }

    // Build the root OrgData node.
    // SAFETY: root_children borrows root_ref which lives in
    // root_arena._bumps (both move into the returned Forest).
    let root_children: BumpVec<'a, NodeId> = unsafe { std::mem::transmute(root_children) };
    let root_id = root_arena.alloc(SyntaxNode {
        parent: None,
        children: root_children,
        data: Syntax::OrgData,
        location: Interval {
            start: 0,
            end: input.len(),
        },
        content_location: None,
        post_blank: 0,
        affiliated: None,
    });

    // Assemble the forest: chunk 0 = root, then data chunks in order.
    let mut forest_chunks: Vec<ChunkArena<'a, 'a>> = Vec::with_capacity(data_results.len() + 1);
    forest_chunks.push(root_arena);
    for (arena, _) in data_results {
        forest_chunks.push(arena);
    }
    let mut forest = Forest::from_chunks(forest_chunks, root_id);

    // Apply cross-chunk parent/child links. Each link is applied
    // sequentially so that the borrow from chunks[p_idx] is dropped
    // before chunks[c_idx] is borrowed.
    for link in &links {
        let p_idx = link.parent.chunk() as usize;
        let c_idx = link.child.chunk() as usize;
        forest.chunks[p_idx][link.parent].children.push(link.child);
        forest.chunks[c_idx][link.child].parent = Some(link.parent);
    }

    forest
}

/// Parse `input` using multiple threads, splitting at all headline
/// boundaries (any level).  Headline starts are greedily packed into
/// byte-balanced groups (≈ cores × chunk_multiplier) so that each
/// `rayon` worker receives a similarly-sized workload.
///
/// Each chunk is parsed with its own `bumpalo::Bump` and a chunk-local
/// `NodeId` space.  A spine pass then assembles the per-chunk arenas
/// into a single [`Forest`] by creating a root node and linking
/// parent/child relationships across chunk boundaries.
/// Default chunk multiplier (cores × 8).  Pass to [`parse_parallel`] to
/// get the current default behaviour.
pub const DEFAULT_CHUNK_MULTIPLIER: usize = 8;

pub fn parse_parallel<'a, E>(
    input: &'a str,
    granularity: ParseGranularity,
    environment: &E,
    chunk_multiplier: usize,
) -> Forest<'a, 'a>
where
    E: environment::Environment + Sync + Clone,
{
    let tab_width = environment.tab_width();
    let radio_targets = collect_radio_targets(input);

    let starts = Cursor::find_all_headline_starts(input);
    let spans = balance_spans(input, &starts, chunk_multiplier);

    let send_results: Vec<SendChunk<'a>> = spans
        .par_iter()
        .enumerate()
        .map(|(i, &span)| {
            parse_chunk_with_id(
                input,
                (i + 1) as u32,
                span,
                granularity,
                environment,
                tab_width,
                radio_targets.clone(),
            )
        })
        .collect();

    let results: Vec<(ChunkArena<'a, 'a>, Vec<NodeId>)> = send_results
        .into_iter()
        .map(|SendChunk(a, t)| (a, t))
        .collect();

    spine_pass(input, results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SyntaxT;
    use crate::environment::DefaultEnvironment;

    fn count_nodes(forest: &Forest<'_, '_>, id: NodeId) -> usize {
        let mut count = 1;
        let node = &forest[id];
        for &child in &node.children {
            count += count_nodes(forest, child);
        }
        count
    }

    fn count_by_type(forest: &Forest<'_, '_>, id: NodeId, typ: SyntaxT) -> usize {
        let mut count = 0;
        if SyntaxT::from(&forest[id].data) == typ {
            count += 1;
        }
        let node = &forest[id];
        for &child in &node.children {
            count += count_by_type(forest, child, typ);
        }
        count
    }

    fn serial_forest<'a>(arena: ChunkArena<'a, 'a>, root: NodeId) -> Forest<'a, 'a> {
        Forest::from_chunks(vec![arena], root)
    }

    #[test]
    fn parallel_equals_serial_empty() {
        let input = "";
        let bump = bumpalo::Bump::new();
        let (a1, r1) =
            Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump).parse_buffer();
        let serial = serial_forest(a1, r1);
        let forest = parse_parallel(
            input,
            ParseGranularity::Object,
            &DefaultEnvironment,
            DEFAULT_CHUNK_MULTIPLIER,
        );
        assert_eq!(
            count_nodes(&serial, serial.root()),
            count_nodes(&forest, forest.root())
        );
    }

    #[test]
    fn parallel_equals_serial_no_headline() {
        let input = "just some text\nand more text\n";
        let bump = bumpalo::Bump::new();
        let (a1, r1) =
            Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump).parse_buffer();
        let serial = serial_forest(a1, r1);
        let forest = parse_parallel(
            input,
            ParseGranularity::Object,
            &DefaultEnvironment,
            DEFAULT_CHUNK_MULTIPLIER,
        );
        assert_eq!(
            count_nodes(&serial, serial.root()),
            count_nodes(&forest, forest.root())
        );
    }

    #[test]
    fn parallel_equals_serial_single_headline() {
        let input = "* Headline\nSome content.\n";
        let bump = bumpalo::Bump::new();
        let (a1, r1) =
            Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump).parse_buffer();
        let serial = serial_forest(a1, r1);
        let forest = parse_parallel(
            input,
            ParseGranularity::Object,
            &DefaultEnvironment,
            DEFAULT_CHUNK_MULTIPLIER,
        );
        assert_eq!(
            count_nodes(&serial, serial.root()),
            count_nodes(&forest, forest.root())
        );
        assert_eq!(
            count_by_type(&serial, serial.root(), SyntaxT::Headline),
            count_by_type(&forest, forest.root(), SyntaxT::Headline),
        );
    }

    #[test]
    fn parallel_equals_serial_two_headlines() {
        let input = "before\n* H1\na\n** H1.1\nb\n* H2\nc\n";
        let bump = bumpalo::Bump::new();
        let (a1, r1) =
            Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump).parse_buffer();
        let serial = serial_forest(a1, r1);
        let forest = parse_parallel(
            input,
            ParseGranularity::Object,
            &DefaultEnvironment,
            DEFAULT_CHUNK_MULTIPLIER,
        );
        assert_eq!(
            count_nodes(&serial, serial.root()),
            count_nodes(&forest, forest.root()),
            "node count mismatch"
        );
        assert_eq!(
            count_by_type(&serial, serial.root(), SyntaxT::Headline),
            count_by_type(&forest, forest.root(), SyntaxT::Headline),
            "headline count mismatch"
        );
        assert_eq!(
            count_by_type(&serial, serial.root(), SyntaxT::Section),
            count_by_type(&forest, forest.root(), SyntaxT::Section),
            "section count mismatch"
        );
    }

    #[test]
    fn parallel_equals_serial_many_headlines() {
        let input = "* A\n** A.1\n*** A.1.1\n* B\nsome text\n* C\n** C.1\n* D\n";
        let bump = bumpalo::Bump::new();
        let (a1, r1) =
            Parser::new(input, ParseGranularity::Object, DefaultEnvironment, &bump).parse_buffer();
        let serial = serial_forest(a1, r1);
        let forest = parse_parallel(
            input,
            ParseGranularity::Object,
            &DefaultEnvironment,
            DEFAULT_CHUNK_MULTIPLIER,
        );
        assert_eq!(
            count_nodes(&serial, serial.root()),
            count_nodes(&forest, forest.root())
        );
        assert_eq!(
            count_by_type(&serial, serial.root(), SyntaxT::Headline),
            count_by_type(&forest, forest.root(), SyntaxT::Headline),
        );
    }
}
