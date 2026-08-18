use org_element::prelude::*;
use std::fs;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("../../corpus/yantar92_config.org");
    let input = fs::read_to_string(path).expect("read file");
    let input = if input.contains('\r') {
        input.replace('\r', "")
    } else {
        input
    };

    // Warmup
    for _ in 0..10 {
        let bump = bumpalo::Bump::new();
        let mut p = Parser::new(&input, ParseGranularity::Object, DefaultEnvironment, &bump);
        p.parse_buffer();
    }

    // Steady-state timed loop
    let start = Instant::now();
    let mut iters = 0u64;
    while start.elapsed().as_secs_f64() < 5.0 {
        let bump = bumpalo::Bump::new();
        let mut p = Parser::new(&input, ParseGranularity::Object, DefaultEnvironment, &bump);
        p.parse_buffer();
        iters += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!(
        "Serial: {iters} iters in {elapsed:.2}s = {:.0} MB/s",
        (input.len() as f64 / 1_000_000.0) * iters as f64 / elapsed
    );
}
