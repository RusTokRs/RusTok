# Performance Benchmarks Guide

This guide covers the performance benchmarking suite for RusToK platform.

## Overview

The benchmarks measure performance characteristics of critical components:

- **State Machine Transitions**: Content and order state transitions
- **Tenant Cache Operations**: Cache read/write throughput
- **Event Bus**: Event publishing and delivery performance
- **Content Operations**: Content workflow and query performance
- **Order Operations**: Order processing and monetary calculations

## Running Benchmarks

### Run all benchmarks

```bash
cargo bench -p rustok-benchmarks
```

### Run specific benchmark group

```bash
# State machine benchmarks
cargo bench -p rustok-benchmarks --bench state_machine

# Tenant cache benchmarks
cargo bench -p rustok-benchmarks --bench tenant_cache

# Event bus benchmarks
cargo bench -p rustok-benchmarks --bench event_bus

# Content operations benchmarks
cargo bench -p rustok-benchmarks --bench content_operations

# Order operations benchmarks
cargo bench -p rustok-benchmarks --bench order_operations
```

### Run specific test

```bash
cargo bench -p rustok-benchmarks --bench state_machine draft_to_review
```

## Benchmark Results

Results are saved in `target/criterion/` directory with HTML reports.

View reports:
```bash
# After running benchmarks
open target/criterion/report/index.html
```

## Benchmark Categories

### 1. State Machine Benchmarks

Measures state transition performance for content and order workflows.

**Key Metrics:**
- Transition latency (Draft→Review, Review→Published, etc.)
- Invalid transition handling (fail-fast performance)
- State cloning overhead

**Example Results:**
```
content_state_transitions/draft_to_review    time:   [450.20 ns 452.30 ns 454.50 ns]
content_state_transitions/review_to_published time:  [380.15 ns 382.25 ns 384.50 ns]
order_state_transitions/cart_to_pending      time:   [520.30 ns 523.40 ns 526.60 ns]
```

### 2. Tenant Cache Benchmarks

Measures cache operation throughput and contention.

**Key Metrics:**
- Cache hit/miss latency
- Insert/invalidate operations
- Concurrent read performance
- Throughput at different cache sizes

**Example Results:**
```
tenant_cache/get_hit                         time:   [45.20 ns 46.10 ns 47.00 ns]
tenant_cache/get_miss                        time:   [38.50 ns 39.20 ns 40.00 ns]
tenant_cache_throughput/100                  time:   [12.50 µs 12.80 µs 13.10 µs]
tenant_cache_throughput/10000                time:   [1.25 ms 1.28 ms 1.31 ms]
```

### 3. Event Bus Benchmarks

Measures event publishing and delivery throughput.

**Key Metrics:**
- Event publishing latency (different payload sizes)
- Throughput (events per second)
- Subscriber delivery performance
- Event filtering overhead

**Example Results:**
```
event_publish/small_event                    time:   [85.20 ns 87.50 ns 89.80 ns]
event_publish/large_event                    time:   [2.50 µs 2.65 µs 2.80 µs]
event_throughput/1000                        time:   [45.20 µs 46.50 µs 47.80 µs]
event_delivery/single_subscriber             time:   [120.50 ns 123.20 ns 126.00 ns]
```

### 4. Content Operations Benchmarks

Measures content workflow and query performance.

**Key Metrics:**
- Full workflow execution time
- State query performance
- Batch operation throughput
- Serialization/deserialization overhead

**Example Results:**
```
content_workflow/full_publish_workflow       time:   [1.20 µs 1.25 µs 1.30 µs]
content_queries/get_history                  time:   [85.20 ns 87.50 ns 89.80 ns]
content_batch/100                            time:   [125.00 µs 128.00 µs 131.00 µs]
```

### 5. Order Operations Benchmarks

Measures order processing and monetary calculation performance.

**Key Metrics:**
- Complete order flow execution
- Order query performance
- High-volume order processing
- Monetary calculation accuracy and speed

**Example Results:**
```
order_workflow/complete_order_flow           time:   [2.50 µs 2.60 µs 2.70 µs]
order_queries/is_paid_paid                   time:   [15.20 ns 15.80 ns 16.40 ns]
order_throughput/1000                        time:   [2.50 ms 2.60 ms 2.70 ms]
```

## Performance Targets

### State Machine
- Single transition: < 500ns
- Full workflow: < 5µs
- State clone: < 100ns

### Tenant Cache
- Cache hit: < 50ns
- Cache miss: < 50ns
- 10K lookups: < 2ms

### Event Bus
- Small event publish: < 100ns
- Large event publish: < 5µs
- 10K events: < 50ms

### Content Operations
- Full publish workflow: < 2µs
- History query: < 100ns
- 100 items batch: < 200µs

### Order Operations
- Complete order flow: < 5µs
- Order query: < 20ns
- 1000 orders batch: < 5ms

## Interpreting Results

### Understanding Criterion Output

```
content_state_transitions/draft_to_review
                        time:   [450.20 ns 452.30 ns 454.50 ns]
                        change: [-2.50% -1.80% -1.10%] (p = 0.02 < 0.05)
                        Performance has improved.
```

- **time**: Average execution time (median and confidence interval)
- **change**: Performance change from baseline (if available)
- **p value**: Statistical significance (p < 0.05 is significant)

### Regression Detection

Criterion automatically detects performance regressions:

```
change: [+5.20% +6.10% +7.00%] (p = 0.001 < 0.05)
Performance has regressed.
```

### Throughput Metrics

Throughput benchmarks measure operations per unit time:

```
Throughput: 100 elements
Time:       [45.20 µs 46.50 µs 47.80 µs]
Ops/sec:    ~2.1 million ops/sec
```

## Profiling Benchmarks

To profile benchmarks and identify bottlenecks:

```bash
# Build with debug symbols
cargo bench --no-run

# Run with perf (Linux)
perf record target/release/deps/state_machine-xxx --bench
perf report

# Run with Instruments (macOS)
instruments -t "Time Profiler" target/release/deps/state_machine-xxx --bench
```

## Continuous Benchmarking

For CI/CD integration, use `--save-baseline` and `--baseline`:

```bash
# Save baseline
cargo bench -p rustok-benchmarks -- --save-baseline main

# Compare against baseline
cargo bench -p rustok-benchmarks -- --baseline main
```

## Best Practices

1. **Run on dedicated hardware** for consistent results
2. **Close other applications** during benchmarking
3. **Run multiple times** to verify consistency
4. **Use `--release` mode** (default for criterion)
5. **Check CPU scaling** - disable for accurate results:
   ```bash
   sudo cpupower frequency-set --governor performance
   ```

## Adding New Benchmarks

1. Create a new file in `ops/benches/benches/`
2. Add benchmark entry to `ops/benches/Cargo.toml`
3. Use `criterion_group!` and `criterion_main!` macros
4. Document the benchmark in this guide

Example:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_my_feature(c: &mut Criterion) {
    c.bench_function("my_feature", |b| {
        b.iter(|| {
            black_box(my_function())
        })
    });
}

criterion_group!(benches, bench_my_feature);
criterion_main!(benches);
```

## Troubleshooting

### Benchmarks run slowly

- Check if running in release mode (default)
- Ensure CPU governor is set to performance
- Check for background processes

### High variance in results

- Increase sample size: `--sample-size 200`
- Check for thermal throttling
- Run on dedicated hardware

### Out of memory

- Reduce batch sizes in benchmarks
- Close other applications
- Use `jemalloc` for better memory management

## Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Target Benchmark Directory Structure](target/criterion/)
