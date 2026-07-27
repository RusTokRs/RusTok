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
- `src/index_storage/` — PostgreSQL evidence tooling for `rustok-index`.
- `src/bin/index_storage_benchmark.rs` — selected JSONB read/query evidence runner.
- `src/bin/index_storage_mutation_benchmark.rs` — transactional update/delete WAL
  evidence runner.
- `src/bin/index_storage_maintenance_benchmark.rs` — committed churn and
  pre/post-VACUUM evidence runner.
- `src/bin/index_partition_snapshot_capture.rs` — owner-operated M3 baseline/shadow
  snapshot runner for the canonical Index relations.

## Purpose

The Criterion suites detect performance regressions in established platform
paths. The M2 Index storage runners exercise only the JSONB layout selected by the
accepted storage ADR. The rejected typed-EAV and hot-projection implementations
were removed after their exact comparison evidence was archived.

The M2 runners create only schemas prefixed with `idx_bench_`:

- `idx_bench_source`
- `idx_bench_jsonb`

Use a dedicated database because those schemas are dropped and recreated on every
M2 run.

The M3 partition snapshot runner is different. It reads the canonical
`index_entities` and `index_links` tables, creates deterministic evidence-ID-bound
shadow parents and children, and copies one repeatable-read snapshot into them. It
does not rename, drop, or alter the canonical production relations. Run it only
against an owner-approved PostgreSQL 16 evidence database.

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

The mutation runner validates the selected JSONB affected entity/link counts,
executes every measured update/delete in an isolated transaction, records full JSON
`EXPLAIN (ANALYZE, BUFFERS, WAL)` output, and rolls the transaction back. The
report exposes maximum per-plan-node WAL records, FPI, and bytes without claiming
they are persistent bloat measurements.

## Index maintenance benchmark

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
INDEX_BENCH_CHURN_CYCLES=5 \
cargo run -p rustok-benchmarks --bin index-storage-maintenance-benchmark --release
```

The maintenance runner commits repeated update plus delete/reinsert cycles, checks
exact entity/link cardinality, and records baseline, after-churn, and
after-`VACUUM (ANALYZE)` schema sizes and `pg_stat_user_tables` counters. Ordinary
VACUUM is used deliberately; the benchmark does not hide an unhealthy model behind
`VACUUM FULL`.

## Index partition snapshot capture

Prepare an immutable partition manifest first. Create a reviewed query audit file:

```json
{
  "contract": "index_partition_query_audit_v1",
  "total_templates": 12,
  "tenant_scoped_templates": 12
}
```

Then run:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_SHADOW_COPY=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_QUERY_AUDIT=evidence/index-partition/query-audit.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
cargo run -p rustok-benchmarks --bin index-partition-snapshot-capture --release
```

The explicit copy opt-in is mandatory. The runner requires PostgreSQL 16 with JIT
disabled, verifies that the canonical relations remain ordinary unpartitioned
tables, serializes one evidence ID through an advisory lock, creates only the
deterministic shadow tables, and copies entities and links from one repeatable-read
snapshot. It adds a shadow-only unique source-version index and validated source
foreign key, calculates stable logical SHA-256 digests, captures child relation
sizes, checks orphan links and post-copy catch-up, and publishes `baseline.json` and
`shadow.json` as a no-clobber pair.

The runner intentionally leaves its evidence-ID-bound shadow tables in place for
later query, mutation, maintenance, and cutover-rehearsal measurements. A failed
run may also leave partial shadow state for operator inspection; use a new manifest
and run key rather than silently reusing or overwriting retained evidence.

Scale values for the M2 runners:

- `smoke`
- `100k`
- `1m`

Optional M2 environment variables:

- `INDEX_BENCH_LOCALES=en-US,ru-RU`
- `INDEX_BENCH_REPETITIONS=3`
- `INDEX_BENCH_CHURN_CYCLES=5`
- `INDEX_BENCH_OUTPUT=target/index-storage-benchmark/report.json`
- `INDEX_BENCH_MUTATION_OUTPUT=target/index-storage-benchmark/mutation-report.json`
- `INDEX_BENCH_MAINTENANCE_OUTPUT=target/index-storage-benchmark/maintenance-report.json`

All reports remain evidence only. JSONB is already selected by
`DECISIONS/2026-07-24-index-storage-layout.md`; production persistence must
implement that accepted envelope rather than importing benchmark DDL or evidence
shadow relations into runtime migrations.
