# M6 Explicit Source Absence Watermark

Status: `source_complete_owner_execution_pending`.

## Purpose

An empty targeted owner load is not proof that an entity is absent. It can also mean that the
adapter cannot observe the row, that retention has already removed a tombstone, or that another
owner boundary was queried incorrectly.

This contract adds one optional, source-owner-published capability for a retained positive-version
absence watermark. It does not change the existing `IndexSource::scan` or `IndexSource::load`
contracts and does not reinterpret their empty results without separate positive evidence.

## Watermark

`IndexSourceAbsenceWatermark` contains only:

- one exact typed `EntityKey`;
- one positive `source_version`.

Construction rejects nil tenant/entity identities, zero schema versions, and zero source versions.
The value contains no payload, reason, actor, timestamp, SQL, database error, transport context, or
raw tombstone record.

The version must come from an owner-retained monotonic record. Current time, row counts, hashes,
cache misses, and an empty ordinary targeted load are not valid substitutes.

## Provider registration

`IndexSourceAbsenceProvider` loads at most one exact watermark and returns:

- `Some(watermark)` when the owner can prove absence at a positive retained version;
- `None` when the owner cannot prove absence;
- one existing bounded retryable/permanent `IndexSourceFailure` when the dependency fails.

Providers register through `IndexSourceAbsenceCatalog` with an owner module, stable provider name,
and exact schema set. Materialization requires the already-frozen `SharedIndexSourceRegistry` and
verifies for every schema that:

- the canonical replay source exists;
- the absence provider owner equals the replay source owner;
- one schema identity does not move between providers across versions;
- provider and schema names remain bounded lowercase machine identifiers.

An absent catalog publishes no false capability. A non-empty catalog without the shared replay
registry fails closed.

## Product locale provider

The selected Product distribution now registers
`product-locale-absence-postgres` for `rustok-product::product@1` and `@2`.

For one exact tenant, Product UUID, and canonical locale, the provider returns the
positive `products.index_revision` only when:

- the live Product row exists;
- the exact `product_translations` row does not exist;
- no retained `product_index_tombstones` row owns that exact locale identity.

Product storage increments `index_revision` when translations are inserted, deleted, or reassigned.
The version is therefore a source-owned high-watermark for the absence of that exact locale. A hard
delete/tombstone remains an ordinary replay `Delete`; a missing Product row, retained tombstone, or
unknown identity returns non-authoritative `None` from this absence provider.

The provider performs one exact PostgreSQL query, returns no payload, and maps storage failures to a
bounded retryable code. It starts no task and writes no owner or Index state.

## Snapshot-reader admission

`IndexDriftDiagnosisOperatorRuntime` materializes the optional absence registry after the canonical
replay source registry is frozen and attaches it privately to
`PostgresIndexDriftSnapshotReader`.

For an empty ordinary targeted load, the reader now:

1. requires an absence provider for the exact schema;
2. requires `Some(watermark)` for the exact requested key and a positive version;
3. represents the source view as typed `Missing`;
4. opens the existing PostgreSQL `REPEATABLE READ READ ONLY` materialized snapshot;
5. reloads the ordinary source and the absence watermark while that snapshot remains open;
6. accepts only an identical `Missing` state with the same positive absence version.

A concurrent translation insert, delete, reassignment, or another owner revision change alters the
second observation and returns retryable `index_drift_source_changed_during_capture`.

The absence version is domain-tagged (`explicit_source_absence_watermark_v1`) and hashed into
the opaque `pg:` boundary only for source `Missing`. Existing Upsert/Delete boundary derivation is
unchanged. Missing provider registration, `None`, cross-scope evidence, zero version, or
malformed provider state remains fail-closed; `index_drift_source_watermark_missing` is
preserved when no authoritative proof exists.

## Source-ready PostgreSQL harness

`product_locale_absence_postgres` now applies the real Product and Index migrations in an isolated
PostgreSQL schema, builds the selected production distribution adapters, admits one stable missing
locale, and deterministically inserts another locale while the real materialized read is blocked.
The race must return retryable `index_drift_source_changed_during_capture`.

The harness copies neither the Product provider SQL nor the source adapter. It remains
`source_ready_owner_execution_pending`; no PostgreSQL result is admitted until the repository owner
runs and retains it.

## Deliberate limits

This slice does not add or claim:

- a ProductVariant, SalesChannel, or arbitrary-module absence provider;
- a shared transaction between Product storage and the Index materialized snapshot;
- tombstone purge policy or retention evidence;
- entity discovery, stale enumeration, or orphan-link diagnosis;
- GraphQL, HTTP, CLI, admin, MCP, or another diagnosis transport;
- finding resolution/ignore commands or repair;
- retained PostgreSQL or production-owner execution evidence.

## Next step

Expose the existing exact-entity diagnosis operator through one bounded server-owned transport that
accepts one typed key, derives authority from request context, and returns only the existing bounded
outcome. Keep discovery, batch scope, lifecycle transitions, and repair outside that transport.
Owner execution and admission of `product_locale_absence_postgres` remain pending.

## Suggested maintainer validation

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
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.