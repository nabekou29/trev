---
paths: "benches/**/*.rs"
---

# Benchmark Conventions

Benchmarks use **Criterion 0.8** with `html_reports` feature.

## File Structure

```rust
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

use criterion::{Criterion, criterion_group, criterion_main};

fn bench_name(c: &mut Criterion) {
    c.bench_function("name", |b| {
        b.iter(|| { /* measured code */ });
    });
}

criterion_group!(benches, bench_name);
criterion_main!(benches);
```

## Key Patterns

- **Stateful benchmarks**: Use `iter_batched` with `BatchSize::SmallInput` or `LargeInput`
- **I/O-heavy benchmarks**: Use custom config with reduced `sample_size(10)` and `measurement_time`
- **Prevent optimization**: Use `std::hint::black_box()` (not `criterion::black_box`)
- **Doc comments**: Not required in bench files
