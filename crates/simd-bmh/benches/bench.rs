// In your benchmark file: `/crates/simd-bmh/benches/bench.rs`

#![feature(maybe_uninit_uninit_array)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use simd_bmh::{Pattern}; // Import the `Pattern` struct and your method
use simd_bmh_macro::parse_pattern; // Import the macro

fn generate_random_bytes(size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.r#gen()).collect()
}

fn insert_pattern_at_percentages(text: &mut [u8], pattern: &[u8], size: usize) {
    for i in (0..=100).step_by(10) {
        let position = (i * size) / 100;
        if position + pattern.len() <= text.len() {
            text[position..position + pattern.len()].copy_from_slice(pattern);
        }
    }
}

fn pattern_benchmark(c: &mut Criterion) {
    // Use the macro to parse the pattern
    const PATTERN: Pattern<4> = parse_pattern!("F? ?3 CC ??");
    let pattern_bytes = &[0xF3, 0xF3, 0xCC, 0x34];

    let mut group = c.benchmark_group("pattern_matching_sizes");

    // Benchmarking different input sizes
    for &size in [
        1024 * 100,    // 100 KB
        1024 * 500,    // 500 KB
        1024 * 1024,   // 1 MB
        1024 * 1024 * 5, // 5 MB
    ]
    .iter()
    {
        let mut text = generate_random_bytes(size);
        insert_pattern_at_percentages(&mut text, pattern_bytes, size);

        // SSE Benchmark
        group.bench_with_input(format!("SSE - Size: {} bytes", size), &size, |b, &_size| {
            b.iter(|| {
                black_box(PATTERN.find_all_matches(black_box(&text)));
            })
        });
    }

    group.finish();
}

criterion_group!(benches, pattern_benchmark);
criterion_main!(benches);
