# Index M3 partition evidence runbook

This runbook defines the owner-operated PostgreSQL evidence procedure required
before tenant-hash partition copy or cutover can be implemented. It does not grant
cutover permission by itself. The canonical `index_entities` and `index_links`
relations remain unpartitioned unless one retained packet validates to `admitted`.

## Tooling boundary

The evidence flow has five explicit boundaries:

1. `partition-prepare` binds an immutable run manifest to one SHA-256
   `evidence_id` and emits deterministic shadow-only bootstrap SQL.
2. `index-partition-snapshot-capture` captures one repeatable-read baseline, creates
   the evidence-ID-bound shadow tables, copies the same snapshot, validates source
   integrity, and publishes `baseline.json` plus `shadow.json`.
3. `index-partition-query-evidence` compares canonical and shadow query behavior in
   one read-only repeatable-read transaction and publishes `query.json`.
4. The owner executes mutation, maintenance, and cutover rehearsal measurements.
   `partition-assemble` reads all six exact raw files, calculates their exact-byte
   SHA-256 identities, and publishes one structurally validated packet.
5. `partition-validate` recalculates admission metrics and atomically publishes
   `admission.json`.

The preparer, snapshot runner, query runner, and assembler refuse to overwrite
retained outputs. The validator removes stale admission output before validation
and writes a new file only after the complete packet is accepted structurally.

None of these tools renames or drops production relations, performs production
cutover, or starts runtime replay or dual-write. The snapshot runner creates and
fills only deterministic shadow relations. The query runner is read-only.

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
strategy, and modulus. Review `bootstrap.sql`. It may contain only shadow parent,
child, and owner-comment statements. Production `ALTER`, `DROP`, `RENAME`, copy,
replay, dual-write, and cutover statements are forbidden.

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
calculates tenant-predicate coverage and requires exactly 10,000 basis points. The
query evidence runner independently requires a tenant identity in every generated
measurement query.

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

The explicit copy opt-in is mandatory. The runner:

- requires PostgreSQL 16 and pins JIT off;
- rejects canonical entity or link tables that are already partitioned;
- validates deterministic shadow names and serializes one evidence ID with an
  advisory lock;
- creates only tenant-hash shadow parents and children;
- reads baseline cardinality and logical digests and copies both canonical relations
  from the same repeatable-read snapshot;
- creates a shadow-only unique source-version index and validated source foreign key;
- records rows, bytes, child sizes, SHA-256 logical digests, orphan count, FK state,
  and post-copy catch-up state;
- publishes `baseline.json` and `shadow.json` together with no-clobber semantics.

The snapshot runner leaves evidence-ID-bound shadow tables in place. A failed
attempt may leave partial state for inspection; use a new manifest and run key
rather than editing or overwriting evidence.

## Capture baseline/shadow query evidence

After the matching snapshot succeeds, run:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_QUERY_EVIDENCE=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
INDEX_PARTITION_QUERY_SAMPLES=7 \
cargo run -p rustok-benchmarks --bin index-partition-query-evidence --release
```

The explicit query opt-in is mandatory. The runner validates:

- the complete canonical manifest identity and deterministic shadow plan;
- PostgreSQL 16, JIT off, and `enable_partition_pruning=on`;
- ordinary unpartitioned canonical relations;
- manifest-bound shadow parent comments, child names, and partition bounds.

It then executes exactly `manifest.repetitions.query` unique tenant-scoped runs in
one read-only repeatable-read transaction. The deterministic template set covers
entity scope pages, keyset pages, exact counts, link scope pages, and source/link
joins when link rows exist. Each run:

- calculates baseline and shadow result rows and SHA-256 result digest parity;
- alternates baseline/shadow execution order across samples;
- retains full JSON `EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` inputs;
- calculates nearest-rank baseline and shadow p95 latency;
- calculates `normalized_partition_plan_v1` SHA-256 plan identities;
- rejects unstable normalized plans or relation reads across samples;
- rejects baseline access to shadow relations and shadow access to canonical tables;
- requires a shadow plan to prune to exactly one entity or link child partition for
  every logical relation used by the query.

The plan normalizer excludes runtime timing, buffers/WAL counters, costs, row
estimates, physical relation/index names, and partition suffixes. It retains the
logical template, tenant predicate contract, predicates, ordering, join type and
algorithm, aggregate/sort/limit structure, and collapsed partition scan shape.
Physical partition fan-out therefore cannot be hidden, while expected
unpartitioned-versus-single-child scan differences do not create false plan drift.

The runner publishes a top-level JSON array to `query.json` using temporary-file and
hard-link no-clobber semantics. Besides the packet-required fields (`name`, p95
latencies, and plan digests), each run retains result parity, relation-read identity,
and every raw EXPLAIN sample for review. It performs no mutation or cutover work.

## Capture the remaining raw artifacts

The complete bundle contains six regular, non-symlink JSON files:

- `baseline.json` — produced by the snapshot runner;
- `shadow.json` — produced by the snapshot runner;
- `query.json` — produced by the query evidence runner;
- `mutation.json` — mutation p95 and WAL measurements;
- `maintenance.json` — ordinary VACUUM/dead-tuple measurements;
- `cutover.json` — lock rehearsals and rollback/invariance facts.

The owner still executes mutation, maintenance, and cutover rehearsal evidence.
Final files are written once and never edited after measurement. A formatting-only
byte change intentionally changes the raw artifact digest.

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

## Assemble the measured packet

```bash
node scripts/verify/index-storage-tooling.mjs partition-assemble \
  --manifest evidence/index-partition/manifest.json \
  --capture evidence/index-partition/capture.json \
  --output evidence/index-partition/partition-packet.json
```

The assembler reads each raw artifact exactly once, hashes its exact bytes, parses
the required shape, constructs `index_partition_evidence_packet_v1`, and runs the
canonical structural validator. It refuses overwrite and does not accept caller
supplied hashes, packet fields, admission reasons, or pass/fail flags.

## Required measured evidence

### Baseline and shadow

Baseline records timestamps, distinct tenants, audited query coverage, and exact
entity/link rows, bytes, and logical digests. Shadow records timestamps, catch-up,
validated FK state, orphan count, logical parity, and one positive size per child.
Partition skew and all admission decisions are calculated later.

### Query measurements

Each unique query run records baseline/shadow p95 and SHA-256 plan digests. Both use
`normalized_partition_plan_v1`. Raw EXPLAIN samples, result digest parity, and exact
child relation reads remain retained reviewer evidence. The validator calculates
maximum non-negative latency regression and counts plan digest changes.

### Mutation and WAL measurements

Each run records baseline/shadow p95 and WAL bytes. The validator calculates maximum
latency regression and WAL amplification.

### Maintenance measurements

Each run records baseline/shadow ordinary VACUUM duration and dead tuples.
`VACUUM FULL` is invalid evidence.

### Cutover rehearsals

Each rehearsal records lock duration, rollback verification, and confirmation that
production relations remained unchanged. This is evidence only; production cutover
is not implemented here.

## Validate and publish admission

```bash
node scripts/verify/index-storage-tooling.mjs partition-validate \
  --input evidence/index-partition/partition-packet.json \
  --output evidence/index-partition/admission.json
```

The validator recomputes manifest identity, provenance, raw hashes, timestamp order,
repetition counts, tenant coverage, digest parity, latency/WAL regression, partition
skew, lock duration, rollback facts, and typed rejection reasons. It publishes
`index_partition_admission_v1` with either `admitted` or `keep_unpartitioned`. A
structurally invalid packet produces no admission output.

## Retention and review

Retain together:

- `config.json`, `manifest.json`, `bootstrap.sql`, and `query-audit.json`;
- all six raw JSON artifacts and their deeper PostgreSQL/EXPLAIN inputs;
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
cargo check -p rustok-benchmarks --bin index-partition-snapshot-capture
cargo check -p rustok-benchmarks --bin index-partition-query-evidence
node --test scripts/verify/index-partition-evidence.test.mjs
node --test scripts/verify/index-partition-evidence-assembly.test.mjs
node scripts/verify/verify-index-partition-evidence.mjs
node scripts/verify/verify-index-partition-snapshot-capture.mjs
node scripts/verify/verify-index-partition-query-evidence.mjs
node scripts/verify/index-storage-tooling.mjs contract
```
