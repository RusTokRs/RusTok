# M6 Explicit Source Absence Watermark

Status: `source_complete_owner_registration_and_reader_wiring_pending`.

## Purpose

An empty targeted owner load is not proof that an entity is absent. It can also mean that the
adapter cannot observe the row, that retention has already removed a tombstone, or that another
owner boundary was queried incorrectly.

This contract adds one optional, source-owner-published capability for a retained positive-version
absence watermark. It does not change the existing `IndexSource::scan` or `IndexSource::load`
contracts and does not reinterpret their empty results.

## Watermark

`IndexSourceAbsenceWatermark` contains only:

- one exact typed `EntityKey`;
- one positive `source_version`.

Construction rejects nil tenant/entity identities, zero schema versions, and zero source versions.
The value contains no payload, reason, actor, timestamp, SQL, database error, transport context, or
raw tombstone record.

The version must come from an owner-retained monotonic record. Examples include a durable delete
tombstone or an explicit high-watermark that is committed atomically with owner identity removal.
Current time, row counts, hashes, cache misses, and an empty ordinary targeted load are not valid
substitutes.

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

## Exact lookup

`SharedIndexSourceAbsenceRegistry::load(key)` performs one bounded provider call. It rejects a
watermark for another tenant, schema, entity, or locale. The registry performs no scan, identifier
collection, database write, scheduling, retry loop, or repair action.

`None` remains non-authoritative. Until the PostgreSQL drift snapshot reader is wired to this
registry, its existing `index_drift_source_watermark_missing` behavior for empty targeted loads is
unchanged.

## Deliberate limits

This slice does not add or claim:

- a production Product, ProductVariant, SalesChannel, or other owner provider;
- reader admission of `Missing` from the new registry;
- a shared transaction between owner storage and the Index materialized snapshot;
- tombstone purge policy or retention evidence;
- entity discovery, stale enumeration, or orphan-link diagnosis;
- GraphQL, HTTP, CLI, admin, MCP, or another transport;
- finding resolution/ignore commands or repair;
- retained PostgreSQL or production-owner execution evidence.

## Next step

Register one production owner-retained provider and wire the frozen absence registry into
`PostgresIndexDriftSnapshotReader`. The reader must compare the exact positive absence version
before and after its PostgreSQL materialized snapshot, bind that version into the opaque consistency
boundary, and continue to fail closed when no watermark is available.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_absence -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-source-absence-watermark.mjs
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.
