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
- `src/index_storage/` — the selected JSONB PostgreSQL regression harness for
  `rustok-index`, retained after the completed M2 comparison.
- `src/bin/index_storage_benchmark.rs` — read/query evidence runner.
- `src/bin/index_storage_mutation_benchmark.rs` — transactional update/delete
  WAL evidence runner.
- `src/bin/index_storage_maintenance_benchmark.rs` — committed churn and
  pre/post-VACUUM evidence runner.

## Purpose

The Criterion suites detect performance regressions in established platform
paths. The Index storage runners now exercise only the JSONB layout selected by
the accepted storage ADR. The rejected typed-EAV and hot-projection implementations
were removed after their exact comparison evidence was archived.

The Index runners create only schemas prefixed with `idx_bench_`:

- `idx_bench_source`
- `idx_bench_jsonb`

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

Before timings are accepted, the runner verifies source/JSONB entity and link
cardinality and identical source/JSONB result digests for all shared workloads.

## Index mutation/WAL benchmark

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-mutation-benchmark --release
```

The mutation runner validates the selected JSONB affected entity/link counts, executes every
measured update/delete in an isolated transaction, records full JSON
`EXPLAIN (ANALYZE, BUFFERS, WAL)` output, and rolls the transaction back. The
report exposes maximum per-plan-node WAL records, FPI, and bytes without
claiming they are persistent bloat measurements.

## Index maintenance benchmark

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
INDEX_BENCH_CHURN_CYCLES=5 \
cargo run -p rustok-benchmarks --bin index-storage-maintenance-benchmark --release
```

The maintenance runner commits repeated update plus delete/reinsert cycles,
checks exact entity/link cardinality, and records baseline, after-churn, and
after-`VACUUM (ANALYZE)` schema sizes and `pg_stat_user_tables` counters. Ordinary
VACUUM is used deliberately; the benchmark does not hide an unhealthy model
behind `VACUUM FULL`.

Scale values:

- `smoke`
- `100k`
- `1m`

Optional environment variables:

- `INDEX_BENCH_LOCALES=en-US,ru-RU`
- `INDEX_BENCH_REPETITIONS=3`
- `INDEX_BENCH_CHURN_CYCLES=5`
- `INDEX_BENCH_OUTPUT=target/index-storage-benchmark/report.json`
- `INDEX_BENCH_MUTATION_OUTPUT=target/index-storage-benchmark/mutation-report.json`
- `INDEX_BENCH_MAINTENANCE_OUTPUT=target/index-storage-benchmark/maintenance-report.json`

All three reports remain benchmark evidence only. JSONB is already selected by
`DECISIONS/2026-07-24-index-storage-layout.md`; production persistence begins in
M3 and must implement that accepted envelope rather than importing benchmark DDL.
