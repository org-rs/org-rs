// Micro-benchmarks comparing regex-based pattern dispatch against
// equivalent byte-check implementations.
//
// Each group has two variants:
//   regex — uses Cursor::looking_at with the current CachedRegex
//   byte  — uses direct byte/memchr operations
//
// This means both approaches coexist in the same binary regardless of
// which one the parser currently uses, so the delta is always visible.

use criterion::{black_box, Criterion};
use memchr::memrchr;
use org_element::blocks::{REGEX_COLON_OR_EOL, REGEX_STARTS_WITH_HASHTAG};
use org_element::cursor::Cursor;
use org_element::headline::REGEX_HEADLINE_SHORT;
use org_element::markup::is_horizontal_rule;
use org_element::markup::REGEX_FIXED_WIDTH;
use regex::Regex;

const ITEM_RE: &str = r"([ \t]*([-+]|(([0-9]+)[.)]))|[ \t]+\*)([ \t]|$)";
use org_element::markup::REGEX_HORIZONTAL_RULE;

// ── inline implementations ───────────────────────────────────────────────────

/// Old: two looking_at calls to identify `# comment` vs `#+keyword/block`.
fn hashtag_is_comment_regex(input: &str, cur2: usize) -> bool {
    let cursor = Cursor::new(input, cur2);
    let end = cursor
        .looking_at(&*REGEX_STARTS_WITH_HASHTAG)
        .map(|m| m.end());
    if let Some(end) = end {
        Cursor::new(input, cur2 + end)
            .looking_at(&*REGEX_COLON_OR_EOL)
            .is_some()
    } else {
        false
    }
}

/// New: skip whitespace to '#', then test the following byte directly.
fn hashtag_is_comment_byte(input: &str, cur2: usize) -> bool {
    let bytes = &input.as_bytes()[cur2..];
    let hash_i = bytes
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(0);
    hash_i < bytes.len()
        && bytes[hash_i] == b'#'
        && matches!(bytes.get(hash_i + 1), None | Some(b' ' | b'\n'))
}

/// Old: looking_at with `[ \t]*:( |$)`.
fn fixed_width_regex(input: &str, cur2: usize) -> bool {
    Cursor::new(input, cur2)
        .looking_at(&*REGEX_FIXED_WIDTH)
        .is_some()
}

/// New: skip whitespace to ':', test the following byte.
fn fixed_width_byte(input: &str, cur2: usize) -> bool {
    let bytes = &input.as_bytes()[cur2..];
    let colon_i = bytes
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(0);
    colon_i < bytes.len()
        && bytes[colon_i] == b':'
        && matches!(bytes.get(colon_i + 1), None | Some(b' ' | b'\n'))
}

/// Old: save cursor, goto_line_begin, looking_at REGEX_HEADLINE_SHORT, restore.
fn on_headline_regex(input: &str, pos: usize) -> bool {
    let mut cursor = Cursor::new(input, pos);
    let saved = cursor.pos();
    cursor.goto_line_begin();
    let result = cursor.looking_at(&*REGEX_HEADLINE_SHORT).is_some();
    cursor.set(saved);
    result
}

/// New: memrchr to find line start (or use pos if already at BOL),
/// count leading '*', check for trailing space/tab — no regex, no cursor mutation.
fn on_headline_byte(input: &str, pos: usize) -> bool {
    let bytes = input.as_bytes();
    let line_start = if pos == 0 || bytes.get(pos.wrapping_sub(1)) == Some(&b'\n') {
        pos
    } else {
        memrchr(b'\n', &bytes[..pos]).map_or(0, |i| i + 1)
    };
    let tail = &bytes[line_start..];
    let n = tail.iter().take_while(|&&b| b == b'*').count();
    n > 0 && tail.get(n).map_or(false, |&b| b == b' ' || b == b'\t')
}

/// Old: looking_at with `[ \t]*-{5,}[ \t]*$`.
fn horizontal_rule_regex(input: &str, pos: usize) -> bool {
    Cursor::new(input, pos)
        .looking_at(&*REGEX_HORIZONTAL_RULE)
        .is_some()
}

/// New: use the shared `is_horizontal_rule` byte-level function.
/// Slices the current line from `pos`, then delegates.
fn horizontal_rule_byte(input: &str, pos: usize) -> bool {
    let line_end = input[pos..]
        .bytes()
        .position(|b| b == b'\n')
        .unwrap_or(input.len() - pos);
    is_horizontal_rule(&input[pos..pos + line_end])
}

/// Old: Cursor::looking_at with REGEX_ITEM.
fn item_regex(input: &str, pos: usize, re: &Regex) -> bool {
    let end = input[pos..]
        .find('\n')
        .map(|p| pos + p)
        .unwrap_or(input.len());
    re.find(&input[pos..end]).is_some_and(|m| m.start() == 0)
}

/// New: scan the line — non-whitespace → bullet type → post-bullet space/EOL.
fn item_byte(input: &str, pos: usize) -> bool {
    let bytes = &input.as_bytes()[pos..];
    let line_end = bytes
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(bytes.len());
    let line = &bytes[..line_end];
    let mut i = 0;

    // Leading whitespace
    while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
        i += 1;
    }
    if i >= line.len() {
        return false;
    }

    // Bullet
    match line[i] {
        b'-' | b'+' => i += 1,
        b'*' => {
            // Must have at least one leading space to be a list item
            if i == 0 {
                return false;
            }
            i += 1;
        }
        _ if line[i].is_ascii_digit() => {
            // Numbered list: digits then '.' or ')'
            while i < line.len() && line[i].is_ascii_digit() {
                i += 1;
            }
            if i >= line.len() || (line[i] != b'.' && line[i] != b')') {
                return false;
            }
            i += 1;
        }
        _ => return false,
    }

    // Must be followed by whitespace or end of line
    i >= line.len() || line[i] == b' ' || line[i] == b'\t'
}

/// Simulate the current paragraph-parser dispatch: `line.trim()` + starts_with checks.
fn paragraph_check_trim(line: &str, end_gt_start: bool) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    if !end_gt_start {
        return false;
    }
    if trimmed.starts_with('*')
        && trimmed.len() > 1
        && matches!(trimmed.as_bytes().get(1), Some(b' ' | b'*'))
    {
        return true; // headline
    }
    if trimmed.starts_with("#+") || trimmed.starts_with("# ") || trimmed == "#" {
        return true; // keyword / comment
    }
    if is_horizontal_rule(trimmed) {
        return true;
    }
    // Inline item check — same logic as starts_with_item.
    paragraph_item_check(line)
}

/// Byte-level dispatch: find first non-whitespace byte, match on it directly,
/// avoiding the full `trim()` scan from both ends.
fn paragraph_check_byte(line: &str, end_gt_start: bool) -> bool {
    let bytes = line.as_bytes();
    let first_non_ws = bytes.iter().position(|&b| b != b' ' && b != b'\t');
    let Some(i) = first_non_ws else {
        return true; // blank line
    };
    if !end_gt_start {
        return false;
    }
    match bytes[i] {
        b'*' if bytes.get(i + 1).is_some_and(|&b| b == b' ' || b == b'*') => true,
        b'#' => {
            i + 1 < bytes.len() && bytes[i + 1] == b'+'
                || bytes.get(i + 1).is_some_and(|&b| b == b' ')
                || i + 1 == bytes.len()
        }
        // Horizontal rule must be checked before item for `-`/`+` lines
        b'-' | b'+' => {
            // If followed by space/tab or end of line it's a list item.
            // If 5+ hyphens with only trailing ws it's an hrule.
            match bytes.get(i + 1) {
                Some(b' ' | b'\t') | None => true,
                _ => is_horizontal_rule(&line[i..]),
            }
        }
        b'0'..=b'9' => paragraph_item_check(line),
        _ => is_horizontal_rule(&line[i..]),
    }
}

/// Inline item check matching `starts_with_item` in list.rs.
fn paragraph_item_check(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }
    match bytes[i] {
        b'-' | b'+' => i += 1,
        b'*' => {
            if i == 0 {
                return false;
            }
            i += 1;
        }
        _ if bytes[i].is_ascii_digit() => {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i >= bytes.len() || (bytes[i] != b'.' && bytes[i] != b')') {
                return false;
            }
            i += 1;
        }
        _ => return false,
    }
    i >= bytes.len() || bytes[i] == b' ' || bytes[i] == b'\t'
}

// ── test corpora ─────────────────────────────────────────────────────────────
//
// Each constant is a block of lines with realistic average length so that
// the memchr scan inside looking_at has genuine work to do.  The mix of
// matching and non-matching lines ensures neither branch is dead code.

const HASHTAG_INPUT: &str = concat!(
    "# This is a comment line with some content following\n",
    "  # Indented comment line — leading spaces then hash\n",
    "#\n",
    "#+BEGIN_SRC rust\n",
    "#+TITLE: A Document Title That Is Reasonably Long\n",
    "#+AUTHOR: Author Name\n",
    "# Another comment\n",
    "#+BEGIN_QUOTE\n",
    "# comment at column zero again\n",
    "#+KEYWORD: some keyword value here\n",
);

const FIXED_WIDTH_INPUT: &str = concat!(
    ": This is a fixed-width line with content\n",
    "  : Indented fixed-width line with more\n",
    ":\n",
    ":PROPERTIES:\n",
    ": More content here on another fixed-width line\n",
    ":END:\n",
    ": last fixed-width entry\n",
    ":LOGBOOK:\n",
);

const HEADLINE_INPUT: &str = concat!(
    "* Top level headline with a typical length title\n",
    "** Second level heading here\n",
    "*** Third level heading with extra content\n",
    "Not a headline — regular paragraph text\n",
    "  * not a headline (leading whitespace)\n",
    "Regular content line in the document.\n",
    "**** Deep fourth-level heading\n",
    "More paragraph text follows here.\n",
    "* Another top-level headline\n",
    "** Subsection under the second headline\n",
);

const HRULE_INPUT: &str = concat!(
    "Not a rule — plain text line with some content\n",
    "----\n",
    "Regular paragraph text that continues here\n",
    "  -----\n",
    "Another descriptive line about the document\n",
    "-----\n",
    "Some text followed by a rule later in the file\n",
    "----------\n",
    "----- \n",
    "  -----  \n",
    "----  \n",
    "-----x\n",
    "  ----- text\n",
    "text -----\n",
    "another line of regular content here\n",
    "  -----something\n",
);

const ITEM_INPUT: &str = concat!(
    "- a simple list item with some descriptive text\n",
    "  - indented item under the previous one\n",
    "+ a plus-bullet item\n",
    "1. ordered list item\n",
    "10) another ordered item with a closing paren\n",
    "regular paragraph text that should not match\n",
    "  * asterisk item with leading space\n",
    "* bare asterisk at start of line is not an item\n",
    "  5. indented ordered item\n",
    "-----\n",
    "another paragraph line for non-match testing\n",
    "  + indented plus item\n",
    "100. deeply numbered item for edge case testing\n",
    "text with - dash in middle\n",
    "  *\n",
);

const PARA_INPUT: &str = concat!(
    "Regular paragraph text that continues for a while without any breaks\n",
    "This is more of the paragraph content flowing across multiple lines\n",
    "Some more descriptive text in the middle of a paragraph body here\n",
    "* Headline that should cause a paragraph break when encountered\n",
    "This line is back in paragraph mode after the headline above\n",
    "  - an indented list item that would break a paragraph\n",
    "And here we are back to regular prose once more in the document\n",
    "1. ordered list item would also break the paragraph here\n",
    "Writing more paragraph text to keep the benchmark realistic\n",
    "#+BEGIN_SRC rust\n",
    "And more text after the keyword line to keep things flowing\n",
    "# Another comment-style line that terminates paragraphs\n",
    "The quick brown fox jumps over the lazy dog near the bank\n",
    "-----\n",
    "More normal text content that doesn't match any patterns\n",
    "  -----  \n",
    "# a comment\n",
);

// Collect the byte offset of every line start in `s`.
fn line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, &b) in s.as_bytes().iter().enumerate() {
        if b == b'\n' && i + 1 < s.len() {
            v.push(i + 1);
        }
    }
    v
}

// ── benchmark groups ─────────────────────────────────────────────────────────

fn bench_hashtag_dispatch(c: &mut Criterion) {
    let input = HASHTAG_INPUT;
    let positions = line_starts(input);

    let mut group = c.benchmark_group("hashtag_dispatch");
    group.sample_size(500);

    group.bench_function("regex", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| hashtag_is_comment_regex(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.bench_function("byte", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| hashtag_is_comment_byte(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.finish();
}

fn bench_fixed_width(c: &mut Criterion) {
    let input = FIXED_WIDTH_INPUT;
    let positions = line_starts(input);

    let mut group = c.benchmark_group("fixed_width");
    group.sample_size(500);

    group.bench_function("regex", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| fixed_width_regex(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.bench_function("byte", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| fixed_width_byte(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.finish();
}

fn bench_on_headline(c: &mut Criterion) {
    let input = HEADLINE_INPUT;
    let positions = line_starts(input);

    let mut group = c.benchmark_group("on_headline");
    group.sample_size(500);

    group.bench_function("regex", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| on_headline_regex(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.bench_function("byte", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| on_headline_byte(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.finish();
}

fn bench_horizontal_rule(c: &mut Criterion) {
    // Verify both implementations agree on every line
    let input = HRULE_INPUT;
    let positions = line_starts(input);
    for &pos in &positions {
        let r = horizontal_rule_regex(input, pos);
        let b = horizontal_rule_byte(input, pos);
        assert_eq!(r, b, "Mismatch at byte {}: regex={} byte={}", pos, r, b);
    }

    let mut group = c.benchmark_group("horizontal_rule");
    group.sample_size(500);

    group.bench_function("regex", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| horizontal_rule_regex(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.bench_function("byte", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| horizontal_rule_byte(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.finish();
}

fn bench_item(c: &mut Criterion) {
    let re = Regex::new(ITEM_RE).unwrap();
    let input = ITEM_INPUT;
    let positions = line_starts(input);

    // Verify agreement
    for &pos in &positions {
        let r = item_regex(input, pos, &re);
        let b = item_byte(input, pos);
        assert_eq!(r, b, "Mismatch at byte {}: regex={} byte={}", pos, r, b);
    }

    let mut group = c.benchmark_group("item_dispatch");
    group.sample_size(500);

    group.bench_function("regex", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| item_regex(black_box(input), p, &re) as usize)
                .sum::<usize>()
        })
    });

    group.bench_function("byte", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| item_byte(black_box(input), p) as usize)
                .sum::<usize>()
        })
    });

    group.finish();
}

fn bench_paragraph_dispatch(c: &mut Criterion) {
    let input = PARA_INPUT;
    let positions = line_starts(input);
    let end_gt_start = true; // simulate second+ paragraph line

    // Verify agreement on every line
    for &pos in &positions {
        let line_end = input[pos..]
            .find('\n')
            .map(|p| pos + p)
            .unwrap_or(input.len());
        let line = &input[pos..line_end];
        let r = paragraph_check_trim(line, end_gt_start);
        let b = paragraph_check_byte(line, end_gt_start);
        assert_eq!(
            r, b,
            "Mismatch at byte {}: line={:?} trim={} byte={}",
            pos, line, r, b
        );
    }

    let mut group = c.benchmark_group("paragraph_dispatch");
    group.sample_size(500);

    group.bench_function("trim", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| {
                    let line_end = input[p..]
                        .find('\n')
                        .map(|p2| p + p2)
                        .unwrap_or(input.len());
                    paragraph_check_trim(black_box(&input[p..line_end]), end_gt_start) as usize
                })
                .sum::<usize>()
        })
    });

    group.bench_function("byte", |b| {
        b.iter(|| {
            positions
                .iter()
                .map(|&p| {
                    let line_end = input[p..]
                        .find('\n')
                        .map(|p2| p + p2)
                        .unwrap_or(input.len());
                    paragraph_check_byte(black_box(&input[p..line_end]), end_gt_start) as usize
                })
                .sum::<usize>()
        })
    });

    group.finish();
}

/// Simulate the `NodeArena::nodes: Vec<SyntaxNode>` allocation pattern:
/// a single Vec that grows as elements are pushed, with ~15 doublings.
const SYNTAX_NODE_SIZE: usize = 264;

fn bench_arena_vec(c: &mut Criterion) {
    use bumpalo::collections::Vec as BumpVec;
    const NODES: usize = 20_000;

    // A mock SyntaxNode sized to match reality (264 bytes).
    #[repr(C)]
    struct MockNode([u8; SYNTAX_NODE_SIZE]);
    const _: () = assert!(std::mem::size_of::<MockNode>() == SYNTAX_NODE_SIZE);

    let mut group = c.benchmark_group("arena_vec");
    group.sample_size(100);

    // std Vec, natural growth — current production behaviour
    group.bench_function("std_vec", |b| {
        b.iter(|| {
            let mut v: Vec<MockNode> = Vec::new();
            for _ in 0..NODES {
                v.push(MockNode([0u8; SYNTAX_NODE_SIZE]));
            }
            black_box(v);
        })
    });

    // std Vec, pre-sized
    group.bench_function("std_vec_presized", |b| {
        b.iter(|| {
            let mut v: Vec<MockNode> = Vec::with_capacity(NODES);
            for _ in 0..NODES {
                v.push(MockNode([0u8; SYNTAX_NODE_SIZE]));
            }
            black_box(v);
        })
    });

    // bump Vec, natural growth
    let bump_ptr = Box::into_raw(Box::new(bumpalo::Bump::new()));
    group.bench_function("bump_vec", |b| {
        b.iter(|| unsafe {
            let bump: &mut bumpalo::Bump = &mut *bump_ptr;
            bump.reset();
            let mut v: BumpVec<'_, MockNode> = BumpVec::new_in(bump);
            for _ in 0..NODES {
                v.push(MockNode([0u8; SYNTAX_NODE_SIZE]));
            }
            black_box(v);
        })
    });

    // bump Vec, pre-sized
    group.bench_function("bump_vec_presized", |b| {
        b.iter(|| unsafe {
            let bump: &mut bumpalo::Bump = &mut *bump_ptr;
            bump.reset();
            let mut v: BumpVec<'_, MockNode> = BumpVec::with_capacity_in(NODES, bump);
            for _ in 0..NODES {
                v.push(MockNode([0u8; SYNTAX_NODE_SIZE]));
            }
            black_box(v);
        })
    });

    unsafe {
        drop(Box::from_raw(bump_ptr));
    }
    group.finish();
}

fn bench_vec_allocation(c: &mut Criterion) {
    use bumpalo::collections::Vec as BumpVec;

    /// Simulate the pattern of SyntaxNode::children Vecs:
    /// 20k Vecs (one per node), each holding 0-15 NodeIds,
    /// matching the distribution of container vs leaf nodes.
    fn generate_children_sizes() -> Vec<usize> {
        // ~70% leaf (0-2 children), ~20% medium (3-8), ~10% many (9-15)
        let mut sizes = Vec::with_capacity(20_000);
        let mut rng = 42u64;
        for _ in 0..20_000 {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let bucket = (rng >> 32) as u32 % 100;
            let n = if bucket < 70 {
                // leaf — 0..=2
                (rng >> 40) as usize % 3
            } else if bucket < 90 {
                // medium — 3..=8
                3 + (rng >> 40) as usize % 6
            } else {
                // many — 9..=15
                9 + (rng >> 40) as usize % 7
            };
            sizes.push(n);
        }
        sizes
    }

    let sizes = generate_children_sizes();

    // ── global-allocator Vec ──────────────────────────────────────────────
    let mut group = c.benchmark_group("vec_children");
    group.sample_size(100);

    group.bench_function("global_std_vec", |b| {
        b.iter(|| {
            let mut arena: Vec<Vec<usize>> = Vec::with_capacity(sizes.len());
            for &n in &sizes {
                let mut v = Vec::with_capacity(n.min(4));
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });

    // ── pre-sized global Vec (upper bound capacity) ──────────────────────
    group.bench_function("global_presized", |b| {
        b.iter(|| {
            let mut arena: Vec<Vec<usize>> = Vec::with_capacity(sizes.len());
            for &n in &sizes {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });

    // ── bump-allocated Vec (bumpalo::collections::Vec) ───────────────────
    // Leak a Bump into a raw pointer so BumpVec borrows can outlive the
    // explicit moves/borrows in the closure.  We reset() the bump between
    // iterations so memory doesn't grow without bound.
    let bump_ptr = Box::into_raw(Box::new(bumpalo::Bump::new()));
    group.bench_function("bump_vec", |b| {
        b.iter(|| unsafe {
            let bump: &mut bumpalo::Bump = &mut *bump_ptr;
            bump.reset();
            let bump_ref: &bumpalo::Bump = bump;
            let mut arena: BumpVec<'_, BumpVec<'_, usize>> = BumpVec::new_in(bump_ref);
            for &n in &sizes {
                let mut v = BumpVec::new_in(bump_ref);
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });

    group.bench_function("bump_vec_presized", |b| {
        b.iter(|| unsafe {
            let bump: &mut bumpalo::Bump = &mut *bump_ptr;
            bump.reset();
            let bump_ref: &bumpalo::Bump = bump;
            let mut arena: BumpVec<'_, BumpVec<'_, usize>> = BumpVec::new_in(bump_ref);
            for &n in &sizes {
                let mut v = BumpVec::with_capacity_in(n, bump_ref);
                for i in 0..n {
                    v.push(i);
                }
                arena.push(v);
            }
            black_box(arena);
        })
    });
    // Recover and drop the leaked Bump (and all its allocations).
    unsafe {
        drop(Box::from_raw(bump_ptr));
    }

    group.finish();
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_hashtag_dispatch(&mut c);
    bench_fixed_width(&mut c);
    bench_on_headline(&mut c);
    bench_horizontal_rule(&mut c);
    bench_item(&mut c);
    bench_paragraph_dispatch(&mut c);
    bench_vec_allocation(&mut c);
    bench_arena_vec(&mut c);
    c.final_summary();
}
