# Index M3 partition evidence runbook

This runbook defines the owner-operated PostgreSQL evidence procedure required
before tenant-hash partition copy or cutover can be implemented. It does not grant
cutover permission by itself. The canonical `index_entities` and `index_links`
relations remain unpartitioned unless one retained packet validates to `admitted`.

## Tooling boundary

The evidence flow has seven explicit boundaries:

1. `partition-prepare` binds an immutable run manifest to one SHA-256
   `evidence_id` and emits deterministic shadow-only bootstrap SQL.
2. `index-partition-snapshot-capture` captures one repeatable-read baseline, creates
   the evidence-ID-bound shadow tables, copies the same snapshot, validates source
   integrity, and publishes `baseline.json` plus `shadow.json`.
3. `index-partition-query-evidence` compares canonical and shadow query behavior in
   one read-only repeatable-read transaction and publishes `query.json`.
4. `index-partition-mutation-evidence` compares rollback-only canonical and shadow
   mutations and publishes `mutation.json`.
5. `index-partition-maintenance-evidence` creates isolated ordinary/partitioned
   clones, commits equivalent churn only there, measures ordinary VACUUM, and
   publishes `maintenance.json`.
6. The owner executes cutover rehearsal measurements. `partition-assemble` reads all
   six exact raw files, calculates exact-byte SHA-256 identities, and publishes one
   structurally validated packet.
7. `partition-validate` recalculates admission metrics and atomically publishes
   `admission.json`.

The preparer and all artifact producers refuse to overwrite retained outputs. None
of the tools renames or drops production relations, performs production cutover, or
starts runtime replay or dual-write. The mutation runner executes writes only under
savepoints and rolls back both each sample and the enclosing transaction. The
maintenance runner commits only to its evidence-only maintenance schema.

## Prepare an immutable run

Create `evidence/index-partition/config.json`:

```json
{
  "contract": "index_partition_evidence_manifest_v1",
  "repository": "RusTokRs/RusTok",
  "commit": "<full lowercase 40-character commit SHA>",
  "run_key": "<stable workflow-run or UUID-like identifier>",
  "postgres_image": "postgres:16",
  "strategy": "tenant_hash",
  "plan_digest_contract": "normalized_partition_plan_v1",
  "modulus": 16,
  "locales": ["en-US", "ru-RU"],
  "repetitions": {
    "query": 3,
    "mutation": 3,
    "maintenance": 3,
    "cutover": 1
  },
  "thresholds": {
    "minimum_total_rows": 1000000,
    "minimum_total_bytes": 4294967296,
    "minimum_distinct_tenants": 16,
    "maximum_query_p95_regression_bps": 500,
    "maximum_mutation_p95_regression_bps": 500,
    "maximum_wal_amplification_bps": 11000,
    "maximum_partition_size_to_mean_bps": 15000,
    "maximum_cutover_lock_ms": 250
  }
}
```

Use an immutable reviewed commit. `run_key` identifies exactly one run attempt and
must not be reused. Modulus must be a power of two from 2 through 128. PostgreSQL
image is pinned to `postgres:16`. Repetition counts are exact and positive.

```bash
node scripts/verify/index-storage-tooling.mjs partition-prepare \
  --input evidence/index-partition/config.json \
  --manifest evidence/index-partition/manifest.json \
  --bootstrap evidence/index-partition/bootstrap.sql
```

`evidence_id` is the SHA-256 digest of canonical manifest input. Shadow relation
names use `tenant_hash_shadow_v1`; the definition hash binds evidence identity,
strategy, and modulus. Review `bootstrap.sql`. Production `ALTER`, `DROP`, `RENAME`,
copy, replay, dual-write, and cutover statements are forbidden.

## Audit tenant-scoped query coverage

Create reviewed `query-audit.json` beside the manifest:

```json
{
  "contract": "index_partition_query_audit_v1",
  "total_templates": 12,
  "tenant_scoped_templates": 12
}
```

The snapshot runner records these values in `baseline.json`; the admission validator
calculates tenant-predicate coverage and requires exactly 10,000 basis points.

## Capture the baseline and shadow snapshot

Run against an owner-approved PostgreSQL 16 evidence database that already contains
the canonical Index migrations and representative data:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_SHADOW_COPY=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_QUERY_AUDIT=evidence/index-partition/query-audit.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
cargo run -p rustok-benchmarks --bin index-partition-snapshot-capture --release
```

The runner requires PostgreSQL 16/JIT off, ordinary unpartitioned canonical tables,
deterministic shadow names, and a PostgreSQL advisory lock. It copies entities and
links from one repeatable-read snapshot, adds shadow-only source-version integrity,
records row/byte/digest/partition/orphan/FK/catch-up evidence, and publishes
`baseline.json` plus `shadow.json` without overwrite.

Query, mutation, maintenance, and cutover artifacts remain separate owner-run
measurements. A failed attempt may leave partial shadow state for inspection;
operators must use a fresh run key rather than silently reusing retained output.

## Capture baseline/shadow query evidence

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_QUERY_EVIDENCE=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
INDEX_PARTITION_QUERY_SAMPLES=7 \
cargo run -p rustok-benchmarks --bin index-partition-query-evidence --release
```

The query runner executes exactly `manifest.repetitions.query` unique tenant-scoped
runs in one read-only repeatable-read transaction. Each run verifies result digest
parity, alternates execution order, retains full JSON
`EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)`, calculates nearest-rank p95 and
`normalized_partition_plan_v1`, and requires each used shadow relation to prune to
exactly one entity or link child partition. It publishes `query.json` once.

## Capture baseline/shadow mutation and WAL evidence

Run only after the matching snapshot succeeds and the evidence-ID-bound shadow
relations remain unchanged:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_MUTATION_EVIDENCE=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
INDEX_PARTITION_MUTATION_SAMPLES=7 \
cargo run -p rustok-benchmarks --bin index-partition-mutation-evidence --release
```

The explicit mutation opt-in is mandatory. Before measuring, the runner revalidates
the canonical manifest identity, PostgreSQL 16, JIT off, partition pruning,
`synchronous_commit=on`, ordinary canonical tables, shadow comments, child names,
and partition bounds. It requires canonical/shadow row-count parity and loads only
byte-for-byte matching generic entity/link anchors.

The runner creates exactly `manifest.repetitions.mutation` unique runs, alternating
entity timestamp touches and link deletes when matching links exist. The full run
uses one rollback-only repeatable-read transaction. Every validation and EXPLAIN
mutation is rolled back to a savepoint, and the outer transaction is rolled back as
well. No mutation is committed.

Each run:

- requires exactly one affected row on baseline and shadow;
- alternates baseline/shadow execution order across samples;
- retains every full JSON `EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` sample;
- calculates nearest-rank baseline and shadow p95 latency;
- records `baseline_wal_bytes` and `shadow_wal_bytes` as the conservative maximum
  per-sample plan-node WAL bytes;
- rejects unstable affected-row or relation-access evidence;
- rejects baseline access to shadows and shadow access to canonical relations;
- requires each shadow mutation target to prune to exactly one child partition.

The runner publishes one top-level array to `mutation.json` using temporary-file and
hard-link no-clobber semantics. WAL values are plan evidence, not persistent bloat
or post-checkpoint storage measurements.

## Capture baseline/shadow ordinary-VACUUM maintenance evidence

Run only after snapshot capture while the manifest-bound shadow catalog remains
unchanged:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_MAINTENANCE_EVIDENCE=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
INDEX_PARTITION_MAINTENANCE_CYCLES=3 \
INDEX_PARTITION_MAINTENANCE_BATCH=128 \
cargo run -p rustok-benchmarks --bin index-partition-maintenance-evidence --release
```

The explicit maintenance opt-in is mandatory because this runner creates logged
relations and commits churn. It revalidates the complete manifest identity,
PostgreSQL 16, JIT off, partition pruning, `synchronous_commit=on`,
`vacuum_cost_delay=0`, ordinary canonical relations, shadow comments, child names,
and partition bounds. It calculates logical canonical/shadow parity before any
clone is created.

The runner creates one deterministic `index_pe_maintenance_<evidence>` schema and
refuses an existing schema. The evidence-only maintenance schema contains two
ordinary baseline heaps and two tenant-hash-partitioned parents with the manifest
modulus. All clones use the source column/default/storage shape but deliberately do
not import production constraints or indexes. Autovacuum is disabled on every
physical clone so the owner-operated run controls cleanup timing.

Canonical and retained snapshot-shadow relations are read-only sources. Every
committed entity timestamp update and link delete/reinsert occurs only in the
maintenance clones. For each of exactly `manifest.repetitions.maintenance` unique
runs, the runner:

- requires the baseline and partitioned clones to start with zero estimated dead
  tuples after the previous ordinary VACUUM;
- commits the configured number of deterministic churn cycles and requires identical
  baseline/shadow affected-row counts;
- verifies exact logical row/digest parity before maintenance;
- flushes PostgreSQL statistics through `pg_stat_force_next_flush`;
- records positive pre-VACUUM `pg_stat_user_tables.n_dead_tup` totals and full
  per-table insert/update/delete/HOT/vacuum/analyze counters;
- alternates baseline/shadow measurement order;
- times ordinary `VACUUM (ANALYZE)` over both baseline heaps and every physical
  shadow partition outside a transaction;
- requires zero estimated dead tuples and equal logical digests after cleanup;
- verifies that canonical and retained snapshot-shadow relations remain unchanged.

The runner publishes a top-level `maintenance.json` array with the packet-required
`baseline_vacuum_ms`, `shadow_vacuum_ms`, `baseline_dead_tuples`, and
`shadow_dead_tuples`, plus detailed before/after statistics and logical digests.
Temporary-file plus hard-link publication refuses overwrite. The evidence schema is
left in place for owner inspection; rerunning the same evidence ID fails closed.

## Capture the remaining raw artifacts

The complete bundle contains six regular, non-symlink JSON files:

- `baseline.json` — produced by the snapshot runner;
- `shadow.json` — produced by the snapshot runner;
- `query.json` — produced by the query evidence runner;
- `mutation.json` — produced by the mutation/WAL evidence runner;
- `maintenance.json` — produced by the ordinary-VACUUM maintenance runner;
- `cutover.json` — lock rehearsals and rollback/invariance facts.

The owner still executes maintenance and cutover rehearsal evidence. Maintenance
tooling is now available, but its real PostgreSQL artifact and the cutover rehearsal
remain owner-run. Final files are written once and never edited after measurement.

Create `capture.json` beside the six artifacts:

```json
{
  "contract": "index_partition_capture_v1",
  "completed_at": "2026-07-27T14:00:00Z",
  "run_provenance": {
    "repository": "RusTokRs/RusTok",
    "commit": "<same full commit SHA as manifest>",
    "run_key": "<same run key as manifest>",
    "job": "index-partition-evidence",
    "runner_os": "Linux",
    "runner_arch": "X64"
  },
  "database": {
    "version": "PostgreSQL 16.x",
    "server_version_num": "160000",
    "jit": "off",
    "system_identifier": "<pg_control_system system_identifier>",
    "database_name": "rustok_index_partition_evidence"
  },
  "artifacts": {
    "baseline": "baseline.json",
    "shadow": "shadow.json",
    "query": "query.json",
    "mutation": "mutation.json",
    "maintenance": "maintenance.json",
    "cutover": "cutover.json"
  }
}
```

Artifact paths are relative to `capture.json` and remain inside the canonical
bundle. Absolute paths, traversal, duplicates, directories, symbolic links, hard
link aliases, and input/output aliases fail closed.

## Retained artifact contract

The packet bundle contains six raw JSON artifacts. They must be regular files, never symbolic links.
The assembler rejects hard-link aliases so two artifact roles cannot reference the same inode.
The PostgreSQL system identifier binds the database instance used for the run.
The assembler calculates SHA-256 digests for the retained raw baseline, shadow,
query, mutation, maintenance, and cutover bytes before parsing their contracts.
Tenant-predicate coverage is calculated by the validator from the audited template
counts. The packet cannot supply a precomputed pass/fail value; `packet_digest` and
all admission metrics are recalculated from retained input.

The reviewed bootstrap file is shadow-only. It must not contain production `ALTER TABLE`, `DROP TABLE`, `RENAME TO`
statements, copy/replay/dual-write instructions, or cutover authorization.

## Assemble and validate

```bash
node scripts/verify/index-storage-tooling.mjs partition-assemble \
  --manifest evidence/index-partition/manifest.json \
  --capture evidence/index-partition/capture.json \
  --output evidence/index-partition/partition-packet.json

node scripts/verify/index-storage-tooling.mjs partition-validate \
  --input evidence/index-partition/partition-packet.json \
  --output evidence/index-partition/admission.json
```

The assembler reads every raw file once, hashes exact bytes, constructs
`index_partition_evidence_packet_v1`, and refuses caller-supplied hashes or pass
flags. The validator recomputes manifest identity, provenance, repetitions, tenant
coverage, digest parity, query/mutation latency, WAL amplification, partition skew,
lock duration, rollback facts, and typed reasons. Structurally invalid evidence
produces no admission output.

## Required measured evidence

### Baseline and shadow

Rows, bytes, tenants, audited query coverage, logical digests, child sizes, catch-up,
FK validation, and orphan state.

### Query measurements

Unique names, baseline/shadow p95, normalized plan digests, result parity, exact
child reads, and retained raw EXPLAIN samples.

### Mutation and WAL measurements

Unique names, exactly one affected row, baseline/shadow p95, positive maximum
per-sample plan-node WAL bytes, exact shadow child pruning, and retained raw EXPLAIN
samples. The validator calculates mutation latency regression and WAL amplification.

### Maintenance measurements

Each unique run records baseline/shadow ordinary VACUUM duration and positive
pre-cleanup dead tuples. Raw evidence additionally retains the controlled churn
shape, full physical-table statistics, zero post-VACUUM dead-tuple state, logical
parity, and proof that source relations were unchanged. `VACUUM FULL` is invalid.

### Cutover rehearsals

Lock duration, rollback verification, and proof that production relations remained
unchanged. Production cutover is not implemented by this runbook.

## Retention and review

Retain together:

- `config.json`, `manifest.json`, `bootstrap.sql`, and `query-audit.json`;
- all six raw JSON artifacts and deeper PostgreSQL/EXPLAIN/statistics inputs;
- the evidence-only maintenance schema until owner review completes;
- `capture.json`, `partition-packet.json`, and `admission.json`;
- PostgreSQL logs and runner metadata.

Do not combine files from different commits, instances, manifests, run keys, or run
attempts. Admission is necessary but not sufficient for production partitioning.
Reviewers must separately approve durable ownership, bounded copy/checkpointing,
constraint/index attachment, catch-up/replay, cutover, rollback, and recovery.

## Suggested repository checks

The repository owner runs:

```bash
cargo test -p rustok-benchmarks partition_snapshot
cargo test -p rustok-benchmarks partition_query
cargo test -p rustok-benchmarks partition_mutation
cargo test -p rustok-benchmarks partition_maintenance
cargo check -p rustok-benchmarks --bin index-partition-snapshot-capture
cargo check -p rustok-benchmarks --bin index-partition-query-evidence
cargo check -p rustok-benchmarks --bin index-partition-mutation-evidence
cargo check -p rustok-benchmarks --bin index-partition-maintenance-evidence
node --test scripts/verify/index-partition-evidence.test.mjs
node --test scripts/verify/index-partition-evidence-assembly.test.mjs
node scripts/verify/verify-index-partition-evidence.mjs
node scripts/verify/verify-index-partition-snapshot-capture.mjs
node scripts/verify/verify-index-partition-query-evidence.mjs
node scripts/verify/verify-index-partition-mutation-evidence.mjs
node scripts/verify/verify-index-partition-maintenance-evidence.mjs
node scripts/verify/index-storage-tooling.mjs contract
```
