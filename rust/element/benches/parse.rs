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
                let mut parser =
                    Parser::new(black_box(&corpus), ParseGranularity::Object, DefaultEnvironment);
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
                let mut parser =
                    Parser::new(black_box(CORPUS), *granularity, DefaultEnvironment);
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
                let mut parser = Parser::new(
                    black_box(slice),
                    ParseGranularity::Element,
                    DefaultEnvironment,
                );
                parser.parse_buffer();
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
    c.final_summary();
}
