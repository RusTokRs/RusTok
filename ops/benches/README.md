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
- `src/bin/index_storage_benchmark.rs` — read/query evidence runner.
- `src/bin/index_storage_mutation_benchmark.rs` — transactional update/delete
  WAL evidence runner.

## Purpose

The Criterion suites detect performance regressions in established platform
paths. The Index storage runners compare temporary PostgreSQL storage candidates
before any production Index migration is selected.

The Index runners create only schemas prefixed with `idx_bench_`:

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

## Index read/query benchmark

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
```

Before timings are accepted, the runner verifies source/candidate entity/link
cardinality and identical result digests for all shared workloads.

## Index mutation/WAL benchmark

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-mutation-benchmark --release
```

The mutation runner validates equal affected entity/link counts, executes every
measured update/delete in an isolated transaction, records full JSON
`EXPLAIN (ANALYZE, BUFFERS, WAL)` output, and rolls the transaction back. The
report exposes maximum per-plan-node WAL records, FPI, and bytes without
claiming they are persistent bloat measurements.

Scale values:

- `smoke`
- `100k`
- `1m`

Optional environment variables:

- `INDEX_BENCH_LOCALES=en-US,ru-RU`
- `INDEX_BENCH_REPETITIONS=3`
- `INDEX_BENCH_OUTPUT=target/index-storage-benchmark/report.json`
- `INDEX_BENCH_MUTATION_OUTPUT=target/index-storage-benchmark/mutation-report.json`

Persistent churn, dead tuples, bloat, and pre/post-VACUUM evidence remain a
separate open M2 phase. Results are evidence only; the production model is
chosen later in an ADR.
