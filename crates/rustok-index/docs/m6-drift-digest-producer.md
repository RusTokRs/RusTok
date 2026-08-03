# M6 bounded drift digest producer

Status: `producer_contract_source_complete_snapshot_reader_and_scope_completion_pending`

## Purpose

The existing drift-finding writer accepts already-computed source and materialized digests. This
slice adds the database-neutral producer between an admitted snapshot reader and that persistence
boundary. It does not invent a snapshot, open source tables, scan unbounded identifiers, or perform
repair.

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
consistency rule. This slice does not define PostgreSQL snapshot export, owner high-watermarks, or
cross-database transaction semantics. A reader that cannot establish one truthful boundary must
return a bounded dependency failure rather than fabricate a pair.

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

## PostgreSQL writer adapter

`PostgresIndexDriftFindingWriter` implements `IndexDriftMismatchRecorder` and maps writer lifecycle
outcomes to `Created`, `Refreshed`, `Reopened`, or `Suppressed` receipts. Storage failure remains a
bounded retryable dependency code; unsupported backend and request/receipt contract failures are
bounded permanent codes.

The current persisted `IndexDriftFindingScope::Entity` requires a locale. Therefore the adapter
fails closed with `index_drift_locale_free_scope_unsupported` for `EntityKey { locale: None }`.
It does not collapse such findings into schema scope or invent a locale. The generic producer itself
supports locale-free keys; extending persisted finding scope is the next required storage-contract
slice.

## Deliberate limits

This slice does not add or claim:

- a production source/materialized snapshot reader;
- PostgreSQL exported-snapshot or owner high-watermark admission;
- automatic entity discovery, full scans, or orphan-link diagnosis;
- locale-free persisted entity findings;
- finding resolution when states converge;
- resolve/ignore commands, actor/reason audit, or authorization;
- targeted/full/shadow repair;
- GraphQL, HTTP, CLI, admin, scheduler, or graceful-shutdown composition;
- retained PostgreSQL or production-source execution evidence.

## Maintainer verification

```bash
cargo test -p rustok-index drift_digest -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-drift-digest-producer.mjs
git diff --check
```

Tests, verifiers, formatting, Cargo commands, PostgreSQL runs, workflows, and CI were not executed by
the implementation agent.
