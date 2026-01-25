# Benchmarks Directory

Performance benchmarks for critical Tome operations.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench --manifest-path src-tauri/Cargo.toml

# Run specific benchmark
cargo bench --manifest-path src-tauri/Cargo.toml search_benchmark

# Generate benchmark report
cargo bench --manifest-path src-tauri/Cargo.toml -- --save-baseline main
```

## Performance Targets

| Operation | Target | Measurement |
|-----------|--------|-------------|
| Simple search query | < 50ms | P50 latency |
| Complex search query | < 100ms | P95 latency |
| Index 1000 pages | < 30s | Total time |
| Cold start | < 500ms | Time to usable UI |
| Page render | < 100ms | Time to first paint |

## Benchmark Structure

```
benches/
├── README.md           # This file
├── search_benchmark.rs # Search performance
├── index_benchmark.rs  # Indexing performance
└── parse_benchmark.rs  # HTML parsing performance
```

## Writing Benchmarks

Use the `criterion` crate for statistical benchmarking:

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn search_simple_query(c: &mut Criterion) {
    // Setup
    let engine = setup_search_engine();

    c.bench_function("search_simple", |b| {
        b.iter(|| {
            engine.search("Vec", 10)
        })
    });
}

fn search_complex_query(c: &mut Criterion) {
    let engine = setup_search_engine();

    c.bench_function("search_complex", |b| {
        b.iter(|| {
            engine.search("impl Iterator for Vec", 10)
        })
    });
}

criterion_group!(benches, search_simple_query, search_complex_query);
criterion_main!(benches);
```

## CI Integration

Benchmarks run on release branches to track performance regressions:

1. Baseline recorded on `main` branch
2. New benchmarks compared against baseline
3. Regressions > 10% flagged for review
