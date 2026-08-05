# M6 bounded drift digest producer

Status: `producer_reader_and_locale_scope_source_complete_host_diagnosis_pending`

## Purpose

The drift-finding writer accepts already-computed source and materialized digests. The
`IndexDriftDigestProducer` is the database-neutral boundary between an admitted snapshot reader and
that persistence adapter. It does not invent a snapshot, open source tables, scan unbounded
identifiers, or perform repair.

## Snapshot contract

`IndexDriftSnapshotReader` captures exactly one requested `EntityKey` and returns two typed views:

- authoritative source state;
- current materialized Index state.

Both views must carry the same bounded opaque `IndexDriftSnapshotBoundary`. Pair construction fails
when boundaries or entity keys differ. The producer independently rechecks both returned keys
against the request before validating or hashing either state.

A state is exactly one of:

- `missing` with the complete entity key;
- `upsert` with a complete `IndexRecord`;
- `delete` with the complete entity key and positive source version.

Every source and materialized state is validated through the current `SchemaRegistry`. Invalid
schema, tenant, entity, locale, field, link, value, or source-version state fails before hashing and
before persistence.

The opaque boundary is evidence that the reader captured both views under one owner-defined
consistency rule. The producer itself still does not define PostgreSQL snapshot export, owner
high-watermarks, or cross-database transaction semantics. A reader that cannot establish one
truthful boundary must return a bounded dependency failure rather than fabricate a pair.

## Digest contract

The producer serializes the typed state with postcard, prefixes the
`index_drift_entity_state_digest_v1` domain, length-prefixes both components, and computes lowercase
SHA-256. `missing`, `delete`, and `upsert` have distinct enum domains. `IndexRecord.fields` retain
`BTreeMap` ordering; link and target order remains significant because it represents materialized
relation ordinal semantics.

Equal digests return `Consistent` and never call the recorder. Unequal digests create one bounded
`IndexDriftDigestMismatch` containing only:

- the exact entity key;
- the opaque consistency-boundary token;
- source digest;
- materialized digest.

Raw records, fields, links, source payloads, SQL, database errors, and transport context are not
accepted by the recorder contract.

## PostgreSQL snapshot reader

`PostgresIndexDriftSnapshotReader` is now source complete. It observes one positive-version owner
state, reads materialized entity/link state inside one PostgreSQL `REPEATABLE READ READ ONLY`
transaction, and accepts the pair only when the complete owner state is identical on a second
observation while that transaction remains open.

The reader returns bounded retryable failure when the owner changes and permanent failure when an
empty targeted load has no retained tombstone or other absence watermark. It does not claim an
exported snapshot across arbitrary owner adapters. Full details are retained in
`m6-postgres-drift-snapshot-reader.md`.

## PostgreSQL writer adapter

`PostgresIndexDriftFindingWriter` implements `IndexDriftMismatchRecorder` and maps writer lifecycle
outcomes to `Created`, `Refreshed`, `Reopened`, or `Suppressed` receipts. Storage failure remains a
bounded retryable dependency code; unsupported backend and request/receipt contract failures are
bounded permanent codes.

The adapter is complete for both valid `EntityKey` locale shapes:

- `locale: Some(locale)` maps to `IndexDriftFindingScope::Entity`;
- `locale: None` maps to `IndexDriftFindingScope::EntityWithoutLocale`.

The locale-free variant persists `locale_key = NULL`; it is never collapsed into schema scope and no
locale is invented. Locale-bearing finding-key bytes retain their original v1 component sequence.
The locale-free scope uses a distinct impossible-for-`LocaleKey` NUL component.

## Deliberate limits

This slice does not add or claim:

- server-owned composition of reader, producer, and writer;
- exported PostgreSQL snapshots shared with arbitrary owner adapters;
- source absence admission without a retained tombstone or explicit positive watermark;
- automatic entity discovery, full scans, or orphan-link diagnosis;
- finding resolution when states converge;
- resolve/ignore commands, actor/reason audit, or authorization;
- targeted/full/shadow repair;
- GraphQL, HTTP, CLI, admin, scheduler, or graceful-shutdown composition;
- retained PostgreSQL or production-source execution evidence.

## Maintainer verification

```bash
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-index --test drift_finding_locale_key_contract

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test drift_snapshot_reader_postgres_test \
  -- --nocapture --test-threads=1

cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-drift-digest-producer.mjs
node scripts/verify/verify-index-drift-snapshot-reader.mjs
node scripts/verify/verify-index-drift-finding-locale-scope.mjs
git diff --check
```

Tests, verifiers, formatting, Cargo commands, PostgreSQL runs, workflows, and CI were not executed by
the implementation agent.
