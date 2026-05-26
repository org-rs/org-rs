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
use org_element::markup::REGEX_FIXED_WIDTH;

// ── inline implementations ───────────────────────────────────────────────────

/// Old: two looking_at calls to identify `# comment` vs `#+keyword/block`.
fn hashtag_is_comment_regex(input: &str, cur2: usize) -> bool {
    let cursor = Cursor::new(input, cur2);
    let end = cursor.looking_at(&*REGEX_STARTS_WITH_HASHTAG).map(|m| m.end());
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

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_hashtag_dispatch(&mut c);
    bench_fixed_width(&mut c);
    bench_on_headline(&mut c);
    c.final_summary();
}
