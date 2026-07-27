# Index M3 partition evidence runbook

This runbook defines the owner-operated PostgreSQL evidence procedure required
before tenant-hash partition copy or cutover can be implemented. It does not grant
cutover permission by itself. The canonical `index_entities` and `index_links`
relations remain unpartitioned unless one retained packet validates to `admitted`.

## Tooling boundary

The evidence flow has four explicit boundaries:

1. `partition-prepare` binds an immutable run manifest to one SHA-256
   `evidence_id` and emits deterministic shadow-only bootstrap SQL.
2. `index-partition-snapshot-capture` captures one repeatable-read baseline, creates
   the evidence-ID-bound shadow tables, copies the same snapshot, validates source
   integrity, and publishes `baseline.json` plus `shadow.json`.
3. The owner executes the remaining query, mutation, maintenance, and cutover
   rehearsal measurements. `partition-assemble` reads all six exact raw files,
   calculates their exact-byte SHA-256 identities, and publishes one structurally
   validated packet.
4. `partition-validate` recalculates admission metrics and atomically publishes
   `admission.json`.

The preparer, snapshot runner, and assembler refuse to overwrite retained outputs.
The validator removes stale admission output before validation and writes a new file
only after the complete packet is accepted structurally.

None of these tools renames or drops production relations, performs production
cutover, or starts runtime replay or dual-write. The snapshot runner creates and
fills only deterministic shadow relations and adds a shadow-only source-version
unique index and validated source foreign key.

## Prepare an immutable run

Create a configuration file such as `evidence/index-partition/config.json`:

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
must not be reused for a rerun. Modulus must be a power of two from 2 through 128.
PostgreSQL image is pinned to `postgres:16`. Repetition counts are exact and must
all be positive.

Prepare the manifest and bootstrap SQL:

```bash
node scripts/verify/index-storage-tooling.mjs partition-prepare \
  --input evidence/index-partition/config.json \
  --manifest evidence/index-partition/manifest.json \
  --bootstrap evidence/index-partition/bootstrap.sql
```

`evidence_id` is the SHA-256 digest of canonical manifest input, including the
commit and unique run key. Shadow relation names use the same
`tenant_hash_shadow_v1` definition contract as `PartitionShadowPlan`: the
definition hash binds evidence identity, strategy, and modulus, and the first 24
hexadecimal characters form the relation suffix.

Review `bootstrap.sql` before execution. It may contain only:

- `CREATE TABLE ... LIKE index_entities ... PARTITION BY HASH (tenant_id)`;
- `CREATE TABLE ... LIKE index_links ... PARTITION BY HASH (tenant_id)`;
- child `PARTITION OF` statements for every remainder;
- owner comments bound to the evidence identity.

It must not contain production `ALTER TABLE`, `DROP TABLE`, `RENAME TO`, copy,
replay, dual-write, or cutover statements.

## Audit tenant-scoped query coverage

Create a reviewed `query-audit.json` beside the manifest:

```json
{
  "contract": "index_partition_query_audit_v1",
  "total_templates": 12,
  "tenant_scoped_templates": 12
}
```

The counts describe the exact query-template set that will later produce
`query.json`. The snapshot runner records these values in `baseline.json`; the
admission validator calculates coverage and requires exactly 10,000 basis points.
The runner does not silently assume full coverage.

## Capture the baseline and shadow snapshot

Run the snapshot binary against an owner-approved PostgreSQL 16 evidence database
that already contains the canonical Index migrations and representative data:

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
- rejects a canonical entity or link table that is already partitioned;
- verifies the deterministic manifest shadow names and serializes one evidence ID
  through a PostgreSQL advisory lock;
- creates only the tenant-hash shadow parents and children;
- reads baseline cardinality and logical digests and copies both canonical relations
  from the same repeatable-read snapshot;
- creates a shadow-only unique source-version index and validated source foreign key;
- records exact row counts, physical bytes, one positive byte size per child,
  SHA-256 logical digests, orphan count, FK state, and post-copy catch-up state;
- publishes `baseline.json` and `shadow.json` together with no-clobber semantics.

The relation digest contract hashes an ordered sequence of per-row PostgreSQL JSON
identities and then binds relation kind and row count to
`index_partition_relation_digest_v1`. It is intended for baseline/shadow parity,
not as a replacement for the retained raw rows or database backup.

The runner intentionally leaves its evidence-ID-bound shadow tables in place.
Query, mutation, maintenance, and cutover artifacts remain separate owner-run
measurements. A failed attempt may leave partial shadow state for inspection; use a
new manifest and run key rather than editing or overwriting the failed evidence.

## Capture all retained raw artifacts

The complete bundle contains six raw JSON artifacts. They are regular files, never
symbolic links:

- `baseline.json` — produced by the snapshot runner;
- `shadow.json` — produced by the snapshot runner;
- `query.json` — an array of baseline/shadow query p95 and normalized plan digests;
- `mutation.json` — an array of mutation p95 and WAL measurements;
- `maintenance.json` — an array of ordinary VACUUM/dead-tuple measurements;
- `cutover.json` — an array of lock rehearsals and rollback/invariance facts.

The artifact producer must write final files once. Do not edit measurements after
the run. A formatting-only byte change intentionally creates a different raw
artifact digest even when parsed JSON values are equal.

Create `capture.json` beside those six artifacts:

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

Artifact paths are relative to `capture.json` and must remain inside the canonical
bundle. Paths and underlying file identities must be unique: absolute paths, `..`
traversal, duplicate roles, directories, symbolic links, and hard-link aliases fail
closed. The manifest, capture descriptor, raw artifacts, and output packet must not
alias each other.

## Assemble the measured packet

Run:

```bash
node scripts/verify/index-storage-tooling.mjs partition-assemble \
  --manifest evidence/index-partition/manifest.json \
  --capture evidence/index-partition/capture.json \
  --output evidence/index-partition/partition-packet.json
```

The assembler reads every raw artifact exactly once, hashes its exact bytes, parses
the required object or array shape, constructs contract
`index_partition_evidence_packet_v1`, and runs the canonical structural validator.
It refuses to overwrite an existing packet and does not accept precomputed raw
hashes, packet fields, admission reasons, or pass/fail flags from `capture.json`.

The packet records:

- runner repository, commit, run key, job, operating system, and architecture;
- PostgreSQL system identifier and database name, so all sections remain bound to
  one database instance;
- SHA-256 digests for the retained raw baseline, shadow, query, mutation,
  maintenance, and cutover artifacts;
- parsed baseline, shadow, query, mutation, maintenance, and cutover evidence.

The repository, commit, and run key in runner provenance must exactly match the
manifest. Raw-artifact roles are exact; missing or additional roles fail
validation.

## Required measured evidence

### Baseline

The unpartitioned baseline records:

- generation timestamp;
- distinct tenant count;
- audited query-template count and tenant-scoped template count;
- exact entity rows, bytes, and SHA-256 logical digest;
- exact link rows, bytes, and SHA-256 logical digest.

Tenant-predicate coverage is calculated by the validator. Admission requires
exactly 10,000 basis points.

### Shadow

The shadow section records:

- generation timestamp and caught-up state;
- validated foreign-key state and orphan-link count;
- exact entity/link rows, bytes, and logical digests;
- one positive byte size for every entity partition;
- one positive byte size for every link partition.

Partition-size skew is calculated from the largest child divided by the mean child
size. The packet cannot supply a precomputed pass/fail value. Timestamps must
satisfy baseline generation before shadow generation before packet completion.

### Query measurements

Each query run records a unique name, baseline and shadow p95 latency, and SHA-256
plan digests. Both digests follow `normalized_partition_plan_v1` and must be
produced by the same reviewed logical plan normalization. Runtime timing,
buffer/WAL counters, and physical relation, alias, and index names are excluded;
operator, join, predicate, ordering, grouping, and partition-pruning semantics are
retained. Raw baseline and shadow
`EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` artifacts remain mandatory retained
inputs for reviewer verification. The validator calculates maximum non-negative
latency regression and counts normalized plan-digest changes.

### Mutation and WAL measurements

Each mutation run records a unique name, baseline and shadow p95 latency, and
baseline/shadow WAL bytes. The validator calculates maximum latency regression and
maximum WAL amplification.

### Maintenance measurements

Each maintenance run records baseline/shadow ordinary VACUUM duration and dead
tuple counts. `VACUUM FULL` is not valid evidence and production health must not
depend on exclusive rewrites.

### Cutover rehearsals

Each rehearsal records measured lock duration plus two independent facts: rollback
was verified and production relations remained unchanged. This evidence is a
rehearsal boundary only; the implementation still contains no production cutover
operation.

## Validate and publish admission

Run:

```bash
node scripts/verify/index-storage-tooling.mjs partition-validate \
  --input evidence/index-partition/partition-packet.json \
  --output evidence/index-partition/admission.json
```

The validator recomputes manifest identity and deterministic relation names,
checks provenance, raw-artifact hashes, timestamp order, exact repetition counts,
and calculates tenant coverage, latency regression, WAL amplification, partition
skew, lock duration, cardinality/digest parity, and all typed rejection reasons.

`admission.json` uses contract `index_partition_admission_v1` and contains both the
manifest `evidence_id` and a SHA-256 `packet_digest` calculated from the complete
canonical packet. It returns either:

- `admitted` with an empty reasons list; or
- `keep_unpartitioned` with calculated typed reasons.

A structurally invalid or incomplete packet produces no admission output.

## Retention and review

Retain together:

- `config.json`;
- `manifest.json`;
- `bootstrap.sql`;
- `query-audit.json`;
- `capture.json`;
- all six raw JSON artifacts and deeper PostgreSQL logs/EXPLAIN inputs from which
  they were derived;
- `partition-packet.json`;
- `admission.json`;
- PostgreSQL logs and runner metadata.

Do not combine files from different commits, PostgreSQL instances, manifests, run
keys, or run attempts. A validated packet is necessary but not sufficient for
production partitioning. Reviewers must still approve durable global operation
ownership, copy/checkpoint semantics, production constraint and index attachment,
catch-up or replay, cutover, rollback, and failure recovery in later changes.

## Suggested repository checks

The repository owner runs:

```bash
cargo test -p rustok-benchmarks partition_snapshot
cargo check -p rustok-benchmarks --bin index-partition-snapshot-capture
node --test scripts/verify/index-partition-evidence.test.mjs
node --test scripts/verify/index-partition-evidence-assembly.test.mjs
node scripts/verify/verify-index-partition-evidence.mjs
node scripts/verify/verify-index-partition-snapshot-capture.mjs
node scripts/verify/index-storage-tooling.mjs contract
```
