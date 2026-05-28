use bumpalo::Bump;
use criterion::{black_box, Criterion};

use org_element::prelude::*;

const CORPUS: &str = include_str!("corpus/large.org");

fn make_corpus(n: usize) -> String {
    let mut s = String::with_capacity(CORPUS.len() * n);
    for _ in 0..n {
        s.push_str(CORPUS);
    }
    s
}

fn bench_parse_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_size");
    group.sample_size(100);

    for (label, n) in [("3k", 1), ("34k", 10), ("340k", 100), ("1.7m", 500)] {
        let corpus = make_corpus(n);
        group.bench_function(label, |b| {
            b.iter(|| {
                let bump = Bump::new();
                let mut parser = Parser::new(
                    black_box(&corpus),
                    ParseGranularity::Object,
                    DefaultEnvironment,
                    &bump,
                );
                parser.parse_buffer();
            })
        });
    }

    group.finish();
}

fn bench_parse_granularity(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_granularity");

    for granularity in &[
        ParseGranularity::Headline,
        ParseGranularity::GreaterElement,
        ParseGranularity::Element,
        ParseGranularity::Object,
    ] {
        let name = format!("{:?}", granularity);
        group.bench_function(&name, |b| {
            b.iter(|| {
                let bump = Bump::new();
                let mut parser =
                    Parser::new(black_box(CORPUS), *granularity, DefaultEnvironment, &bump);
                parser.parse_buffer();
            })
        });
    }

    group.finish();
}

fn bench_parse_viewport(c: &mut Criterion) {
    let corpus = make_corpus(500);
    let len = corpus.len();
    let start_base = len / 4;
    let bytes = corpus.as_bytes();

    let mut group = c.benchmark_group("parse_viewport");
    group.sample_size(100);

    for (label, viewport_size) in [("2k", 2048), ("8k", 8192), ("32k", 32768), ("128k", 131072)] {
        let slice_start = match bytes[start_base..].iter().position(|&b| b == b'\n') {
            Some(pos) => start_base + pos + 1,
            None => 0,
        };

        let end_byte = (start_base + viewport_size).min(len);
        let slice_end = match bytes[end_byte..].iter().position(|&b| b == b'\n') {
            Some(pos) => (end_byte + pos + 1).min(len),
            None => len,
        };

        if slice_start >= slice_end {
            continue;
        }

        let slice = &corpus[slice_start..slice_end];

        group.bench_function(label, |b| {
            b.iter(|| {
                let bump = Bump::new();
                let mut parser = Parser::new(
                    black_box(slice),
                    ParseGranularity::Element,
                    DefaultEnvironment,
                    &bump,
                );
                parser.parse_buffer();
            })
        });
    }

    group.finish();
}

/// 500 headlines each carrying 3 tags, separated by 8 lines of body text.
/// Stresses: tag-Vec allocation per headline (#1) and find_headline_end (#5).
fn make_headline_heavy(count: usize) -> String {
    let mut s = String::with_capacity(count * 120);
    for i in 0..count {
        s.push_str(&format!(
            "* Headline {} :alpha:beta:gamma:\nSome body text on line one.\nLine two of body.\nLine three.\nLine four.\nLine five.\nLine six.\nLine seven.\nLine eight.\n",
            i
        ));
    }
    s
}

/// 3 lists each with `items_per_list` items, some nested two levels deep.
/// Stresses: Rc<ListStruct> cloning per item (#3).
fn make_list_heavy(items_per_list: usize) -> String {
    let mut s = String::with_capacity(items_per_list * 40);
    for _ in 0..3 {
        for i in 0..items_per_list {
            if i % 10 == 5 {
                s.push_str(&format!("- outer item {}\n  - nested item {}\n  - nested item {} b\n", i, i, i));
            } else {
                s.push_str(&format!("- list item {}\n", i));
            }
        }
        s.push('\n');
    }
    s
}

/// Many footnote definitions back-to-back, stressing footnote_definition_parser (#2).
fn make_footnote_heavy(count: usize) -> String {
    let mut s = String::with_capacity(count * 60);
    for i in 0..count {
        s.push_str(&format!(
            "[fn:note{}] This is the body of footnote number {}.\n\n",
            i, i
        ));
    }
    s
}

fn bench_footnote_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("footnote_heavy");
    group.sample_size(100);

    for (label, n) in [("50", 50), ("200", 200), ("1000", 1000)] {
        let corpus = make_footnote_heavy(n);
        group.bench_function(label, |b| {
            b.iter(|| {
                let bump = bumpalo::Bump::new();
                let mut parser = Parser::new(
                    black_box(&corpus),
                    ParseGranularity::Object,
                    DefaultEnvironment,
                    &bump,
                );
                parser.parse_buffer();
            })
        });
    }

    group.finish();
}

fn bench_headline_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("headline_heavy");
    group.sample_size(100);

    for (label, n) in [("50", 50), ("200", 200), ("1000", 1000)] {
        let corpus = make_headline_heavy(n);
        group.bench_function(label, |b| {
            b.iter(|| {
                let bump = Bump::new();
                let mut parser = Parser::new(
                    black_box(&corpus),
                    ParseGranularity::Object,
                    DefaultEnvironment,
                    &bump,
                );
                parser.parse_buffer();
            })
        });
    }

    group.finish();
}

fn bench_list_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_heavy");
    group.sample_size(100);

    for (label, n) in [("50", 50), ("200", 200), ("1000", 1000)] {
        let corpus = make_list_heavy(n);
        group.bench_function(label, |b| {
            b.iter(|| {
                let bump = Bump::new();
                let mut parser = Parser::new(
                    black_box(&corpus),
                    ParseGranularity::Object,
                    DefaultEnvironment,
                    &bump,
                );
                parser.parse_buffer();
            })
        });
    }

    group.finish();
}

/// Parse once outside the loop, then repeatedly walk every node in the tree.
/// Isolates the `Nodes` iterator cost from parsing cost.
fn bench_traverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("traverse");
    group.sample_size(100);

    for (label, n) in [("3k", 1), ("34k", 10), ("340k", 100)] {
        let corpus = make_corpus(n);
        let bump = Bump::new();
        let mut parser =
            Parser::new(&corpus, ParseGranularity::Object, DefaultEnvironment, &bump);
        let (arena, root) = parser.parse_buffer();

        group.bench_function(label, |b| {
            b.iter(|| {
                let count: usize = arena.nodes(black_box(root)).count();
                black_box(count)
            })
        });
    }

    group.finish();
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_parse_sizes(&mut c);
    bench_parse_granularity(&mut c);
    bench_parse_viewport(&mut c);
    bench_headline_heavy(&mut c);
    bench_list_heavy(&mut c);
    bench_footnote_heavy(&mut c);
    bench_traverse(&mut c);
    c.final_summary();
}
