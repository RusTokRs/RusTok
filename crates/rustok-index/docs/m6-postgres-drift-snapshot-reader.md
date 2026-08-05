# M6 PostgreSQL drift snapshot reader

Status: `source_complete_owner_execution_pending`.

## Purpose

`PostgresIndexDriftSnapshotReader` captures one exact owner state and one exact materialized Index
state for a requested `EntityKey` without scanning identifiers, writing Index data, changing a
finding, or choosing repair policy.

The reader is constructed only after the immutable `SharedIndexSourceRegistry` and
`SharedIndexSchemaRegistry` exist. When an explicit `SharedIndexSourceAbsenceRegistry` was
materialized from the same owner-bound composition, the reader attaches it as an optional private
capability.

## Consistency boundary

The generic owner adapter does not expose a caller-owned transaction, so the reader uses an explicit
version fence rather than claiming a cross-database snapshot:

1. targeted-load the exact key from the immutable owner source registry;
2. accept one positive-version `Upsert` or `Delete` mutation; otherwise require one exact positive
   `IndexSourceAbsenceWatermark` and represent the owner state as typed `Missing`;
3. open one PostgreSQL `REPEATABLE READ READ ONLY` transaction;
4. capture `txid_current_snapshot()` and reconstruct the exact materialized entity and links through
   that transaction;
5. targeted-load the owner source again while the transaction remains open and, for `Missing`,
   reload the exact absence watermark;
6. accept only when the complete typed owner state and its evidence version are identical.

A changed mutation, a newly appearing row, a disappearing row, or a changed absence version returns
retryable `index_drift_source_changed_during_capture`.

Missing provider registration, provider `None`, cross-scope evidence, zero versions, or malformed
state remains permanent `index_drift_source_watermark_missing` or the bounded source-contract
failure. An empty targeted load alone is never converted to `Missing`.

## Boundary token

Every accepted pair receives one opaque bounded `pg:` token with lowercase SHA-256 over
length-prefixed components:

- `index_drift_postgres_source_version_boundary_v1`;
- PostgreSQL `txid_current_snapshot()` text;
- the postcard-encoded typed owner state.

For source `Missing`, the hash additionally includes the domain tag
`explicit_source_absence_watermark_v1` and the positive absence `source_version`. Existing
Upsert/Delete boundary derivation is unchanged.

The token is not a credential, replay cursor, SQL string, transaction handle, repair authorization,
or owner payload.

## Materialized reconstruction

The reader queries one exact `index_entities` key. A missing row becomes materialized `Missing`.
Existing rows must retain:

- positive stored source version;
- exact registered schema fingerprint;
- no payload or links for deleted rows;
- decodable typed fields for live rows;
- exact source-version-fenced link rows;
- valid target schemas, UUIDs, locales, and contiguous ordinals;
- registered schema link order.

The digest producer independently validates both returned states through `SchemaRegistry` before
hashing.

## Runtime composition

`IndexDriftDiagnosisOperatorRuntime` freezes the optional absence registry after the canonical replay
source registry, attaches it to the snapshot reader, and privately composes the reader with
`IndexDriftDigestProducer` and `PostgresIndexDriftFindingWriter`.

Composition performs no source or Index SQL and starts no task. The operator exposes only one
authorized exact-entity diagnosis call and no registry, reader, writer, connection, scan, lifecycle,
or repair handle.

## Production Product proof

The selected Product distribution publishes one locale-absence provider. It uses the positive
`products.index_revision` only when the Product exists, the requested translation locale is absent,
and no exact retained Product tombstone owns that locale. Product translation changes advance the
same revision, so the reader's second observation detects concurrent locale appearance or movement.

Hard-delete tombstones remain ordinary source `Delete` values and are not collapsed into `Missing`.

## Evidence status

The existing environment-gated `drift_snapshot_reader_postgres_test` covers stable retained state,
source change, and missing-watermark rejection with a database-neutral sequenced source.

The source-ready `product_locale_absence_postgres` harness now applies the real Product and Index
migrations, materializes the production Product source and Product locale provider, captures stable
source `Missing`, and uses a PostgreSQL table lock plus `pg_stat_activity` to insert a translation
between the two real owner observations. The changed observation must return retryable
`index_drift_source_changed_during_capture`.

No retained PostgreSQL execution evidence is claimed until the repository owner runs and admits
both harnesses.

## Deliberate limits

This slice does not add or claim:

- exported snapshots shared with arbitrary owner adapters;
- ProductVariant, SalesChannel, or arbitrary-module absence providers;
- entity discovery, full scans, stale enumeration, or orphan diagnosis;
- automatic convergence resolution;
- resolve/ignore commands, actor/reason audit, or public transport;
- targeted, full, dry-run, or shadow repair;
- retained execution evidence.

## Maintainer verification

```bash
cargo test -p rustok-index source_absence -- --nocapture
cargo test -p rustok-distribution product_index --features mod-product -- --nocapture
cargo test -p rustok-server index_drift_diagnosis_operator -- --nocapture
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution \
  --features mod-product \
  --test product_locale_absence_postgres \
  -- --nocapture --test-threads=1
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index --test drift_snapshot_reader_postgres_test \
  -- --nocapture --test-threads=1
node scripts/verify/verify-index-product-absence-postgres-harness.mjs
node scripts/verify/verify-index-source-absence-watermark.mjs
node scripts/verify/verify-index-drift-snapshot-reader.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI were
not executed by the implementation agent.