# M6 PostgreSQL drift snapshot reader

Status: `source_complete_host_diagnosis_composition_and_owner_execution_pending`.

## Purpose

`PostgresIndexDriftSnapshotReader` is the first production implementation of the database-neutral
`IndexDriftSnapshotReader` contract. It captures one exact owner state and one exact materialized
Index state for a requested `EntityKey` without scanning identifiers, writing Index data, changing a
finding, or choosing repair policy.

The reader is constructed only after the immutable `SharedIndexSourceRegistry` and
`SharedIndexSchemaRegistry` exist. It therefore uses the same selected owner source and schema
contracts as replay and reconciliation.

## Consistency boundary

The generic `IndexSource` API does not expose a caller-owned database transaction. The reader does
not pretend that two unrelated source and Index reads share one transaction. Instead it implements
an explicit source-version fence:

1. targeted-load the exact key from the immutable owner source registry;
2. require exactly one `Upsert` or `Delete` mutation with a positive source version;
3. open one PostgreSQL `REPEATABLE READ READ ONLY` transaction;
4. capture `txid_current_snapshot()` and read the exact materialized entity and links through that
   same `DatabaseTransaction`;
5. targeted-load the owner source again while the PostgreSQL transaction remains open;
6. accept the pair only when the complete typed owner state is identical to the first observation.

A source change returns retryable `index_drift_source_changed_during_capture`. A source load that
returns no mutation has no admitted absence watermark and returns permanent
`index_drift_source_watermark_missing`; unproven absence is never converted to `Missing`.

This fence is truthful for retained positive-version owner states. It is not equivalent to an
exported PostgreSQL snapshot spanning the owner adapter, and the boundary is not advertised as one.

## Boundary token

The accepted pair receives one opaque bounded token with prefix `pg:` and lowercase SHA-256 over
length-prefixed components:

- `index_drift_postgres_source_version_boundary_v1`;
- the PostgreSQL `txid_current_snapshot()` text captured by the read-only transaction;
- the complete postcard-encoded owner `IndexDriftEntityState` observed before and after the
  materialized read.

The token is evidence of the exact accepted fence. It is not a database credential, replay cursor,
SQL string, transaction handle, repair authorization, or owner payload.

## Materialized state reconstruction

The reader queries one exact `index_entities` key. A missing row becomes materialized `Missing`.
Existing rows must satisfy all of these checks before they cross the reader boundary:

- positive stored source version;
- exact registered schema fingerprint;
- deleted rows have no payload and no links;
- live rows contain a decodable `BTreeMap<FieldName, IndexValue>` payload;
- link rows match the exact source key and source version;
- link and target names, schema versions, UUIDs, and locales decode into typed domain values;
- ordinals for each link are contiguous from zero;
- stored link names are declared by the registered schema;
- output link ordering follows registered schema order and each target vector follows ordinal order.

The existing producer validates both resulting states through `SchemaRegistry` before hashing.

## Runtime composition

`materialize_postgres_index_drift_snapshot_reader` performs no SQL and starts no task. It returns
`None` when the selected distribution has no source registry, fails closed when the schema registry
is missing or the backend is not PostgreSQL, and otherwise constructs the reader from immutable
registries plus the host connection.

The reader is exported through `rustok_index`, but this slice does not insert it into the server
operator, call the digest producer, persist a finding, or expose a transport.

## Source-ready PostgreSQL harness

`drift_snapshot_reader_postgres_test` is environment-gated by
`RUSTOK_INDEX_TEST_DATABASE_URL` with `DATABASE_URL` fallback. It creates an isolated schema,
applies every real Index migration, persists a real schema and materialized entity through the
canonical stores, and retains source code for three cases:

- stable owner version around a materialized mismatch produces one `pg:` snapshot pair;
- owner state changing between the two observations is rejected as retryable;
- source absence without a tombstone or other watermark is rejected as permanent.

The harness is retained execution evidence only after the repository owner runs and admits it.

## Deliberate limits

This slice does not add or claim:

- exported PostgreSQL snapshots shared with arbitrary owner adapters;
- an owner high-watermark protocol beyond an exact positive-version mutation;
- missing-source admission without a retained delete/tombstone;
- entity discovery, full scans, stale enumeration, or orphan diagnosis;
- automatic convergence resolution;
- resolve/ignore commands, actor/reason audit, or public transport;
- targeted, full, dry-run, or shadow repair;
- retained execution evidence.

## Maintainer verification

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test drift_snapshot_reader_postgres_test \
  -- --nocapture --test-threads=1

node scripts/verify/verify-index-drift-snapshot-reader.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI were
not executed by the implementation agent.
