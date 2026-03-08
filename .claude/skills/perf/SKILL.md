---
name: perf
description: Performance optimization workflow. Use when the task involves performance improvement, optimization, profiling, or benchmarking.
---

# Performance Optimization Workflow

**Principle: Always measure before optimizing — never optimize based on guesses.**

## Measurement

### 1. Profiling (primary)

```bash
cargo build --profile profiling
samply record ./target/profiling/trev [args]
```

Analyze profiler output and identify bottlenecks before making changes.

### 2. Benchmarking (hot path comparison)

```bash
mise run bench
```

- Criterion benchmarks (`benches/` directory)
- Compare results before and after changes

## Workflow

1. **Measure**: Establish baseline with profiler or benchmarks
2. **Analyze**: Identify bottlenecks
3. **Optimize**: Improve only the identified areas
4. **Verify**: Re-measure to confirm improvement
