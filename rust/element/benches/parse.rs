use criterion::{black_box, criterion_group, criterion_main, Criterion};

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

criterion_group!(benches, bench_parse_sizes, bench_parse_granularity);
criterion_main!(benches);
