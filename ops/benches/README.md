# Benchmarks (`ops/benches`)

This is a standalone workspace crate named `rustok-benchmarks`.

## What is here

- `Cargo.toml` — benchmark crate manifest.
- `benches/*.rs` — Criterion benchmark suites:
  - `tenant_cache.rs`
  - `state_machine.rs`
  - `event_bus.rs`
  - `content_operations.rs`
  - `order_operations.rs`
- `src/index_storage/` — the M2 PostgreSQL physical-storage comparison for
  `rustok-index`.
- `src/bin/index_storage_benchmark.rs` — executable evidence runner.

## Purpose

The Criterion suites detect performance regressions in established platform
paths. The Index storage runner is different: it compares temporary PostgreSQL
storage candidates before any production Index migration is selected.

The Index runner creates only schemas prefixed with `idx_bench_`:

- `idx_bench_source`
- `idx_bench_jsonb`
- `idx_bench_eav`
- `idx_bench_hot`

Use a dedicated database because those schemas are dropped and recreated on
every run.

## Typical Criterion usage

```bash
cargo bench -p rustok-benchmarks
```

## Index storage benchmark

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
```

Scale values:

- `smoke`
- `100k`
- `1m`

Optional environment variables:

- `INDEX_BENCH_LOCALES=en-US,ru-RU`
- `INDEX_BENCH_REPETITIONS=3`
- `INDEX_BENCH_OUTPUT=target/index-storage-benchmark/report.json`

The generated report contains load times, schema sizes, PostgreSQL settings,
executed SQL, and repeated full JSON `EXPLAIN (ANALYZE, BUFFERS, WAL)` plans.
Results are evidence only; the production model is chosen later in an ADR.
