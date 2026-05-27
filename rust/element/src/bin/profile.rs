use std::fs;
use std::time::Instant;
use org_element::prelude::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = if args.len() > 1 { &args[1] } else { "benches/corpus/large.org" };

    let input = fs::read_to_string(path).expect("read corpus file");

    // Build a ~10MB corpus by repeating it
    let target_size = 10_000_000;
    let repeats = target_size / input.len() + 1;
    let mut corpus = String::with_capacity(input.len() * repeats);
    for _ in 0..repeats {
        corpus.push_str(&input);
    }
    corpus.shrink_to_fit();

    let size_mb = corpus.len() as f64 / 1_000_000.0;
    eprintln!("Corpus: {:.1} MB ({:.0} files)", size_mb, repeats);

    // Warmup
    let mut parser = Parser::new(&corpus, ParseGranularity::Element, DefaultEnvironment);
    parser.parse_buffer();

    let granularities = [
        ("Headline", ParseGranularity::Headline),
        ("GreaterElement", ParseGranularity::GreaterElement),
        ("Element", ParseGranularity::Element),
        ("Object", ParseGranularity::Object),
    ];

    for (label, granularity) in &granularities {
        let start = Instant::now();
        let mut iters = 0u64;
        while start.elapsed().as_secs_f64() < 3.0 {
            let mut parser = Parser::new(&corpus, *granularity, DefaultEnvironment);
            parser.parse_buffer();
            iters += 1;
        }
        let elapsed = start.elapsed().as_secs_f64();
        let throughput = size_mb * iters as f64 / elapsed;
        eprintln!("{label:20} {iters:4} iters in {elapsed:.2}s = {throughput:.0} MB/s");
        // Print for machine parsing
        println!("{label}\t{iters}\t{elapsed:.4}\t{throughput:.0}");
    }
}
