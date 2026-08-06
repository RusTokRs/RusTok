# M6 bounded drift digest producer

Status: `producer_missing_candidate_and_guarded_source_page_source_complete`

## Purpose

The drift-finding writer accepts already-computed source and materialized digests.
`IndexDriftDigestProducer` is the database-neutral boundary between an admitted snapshot reader and
that persistence adapter. It does not invent a snapshot, open source tables, scan unbounded
identifiers, choose finding lifecycle policy, or perform repair.

The producer now exposes two intentionally separate operations:

- `produce(request)` preserves the existing general exact-state digest behavior;
- `produce_missing_entity_candidate(request)` records only authoritative source `Upsert` plus
  materialized `Missing`.

The second operation also has `produce_missing_entity_candidate_from_pair(request, pair)` for one
already-captured pair. This keeps state classification database-neutral and avoids a second snapshot
capture when a caller already owns one admitted exact pair.

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
schema, tenant, entity, locale, field, link, value, or source-version state fails before
classification, hashing, and persistence.

The opaque boundary is evidence that the reader captured both views under one owner-defined
consistency rule. The producer itself does not define PostgreSQL snapshot export, owner
high-watermarks, or cross-database transaction semantics. A reader that cannot establish one
truthful boundary must return a bounded dependency failure rather than fabricate a pair.

## General digest contract

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

Raw records, fields, links, source payloads, SQL, database errors, credentials, transaction handles,
and transport context are not accepted by the recorder contract.

## Missing-only candidate contract

`IndexDriftMissingEntityCandidateOutcome` contains only:

- `NotCandidate`; or
- `MissingRecorded` with two bounded digests and one finding receipt.

All captured state combinations are validated first. The recorder is called only when:

- source state is authoritative `Upsert`; and
- materialized state is exact `Missing`.

Source `Missing`, source `Delete`, materialized `Upsert`, and materialized `Delete` all return
`NotCandidate`, including unequal stale-field, stale-link, and version combinations. The outcome does
not reveal which non-candidate state combination was observed and does not expose raw snapshot
states.

The existing general `produce(request)` behavior remains unchanged and still records every unequal
validated exact-state pair. This prevents missing-only discovery policy from silently changing
caller-known exact diagnosis semantics.

## PostgreSQL snapshot reader

`PostgresIndexDriftSnapshotReader` is source complete. It observes one exact positive-version owner
state, reads materialized entity/link state inside one PostgreSQL `REPEATABLE READ READ ONLY`
transaction, and accepts the pair only when the complete owner state is identical on a second
observation while that transaction remains open.

The reader returns bounded retryable failure when the owner changes. An empty targeted load remains
permanent `index_drift_source_watermark_missing` until the owner can provide a retained tombstone or
another explicit positive absence watermark. The reader does not claim an exported snapshot across
arbitrary owner adapters. Full details are retained in `m6-postgres-drift-snapshot-reader.md`.

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

## Guarded server diagnosis

The server privately composes the production reader, this producer, and the writer inside
`IndexDriftDiagnosisOperatorRuntime`. The capability is published only after the immutable
`SharedIndexSourceRegistry` and `SharedIndexSchemaRegistry` have been frozen by replay composition.

`diagnose_entity(context, key)` preserves general exact diagnosis. The sibling
`diagnose_missing_entity_candidate(context, key)` uses the missing-only producer path. Both require
effective `Permission::MODULES_MANAGE`, reject cross-tenant keys, and authorize before
`IndexDriftDigestRequest` validation, source access, materialized reads, hashing, or finding
persistence.

The bounded GraphQL mutation remains attached only to general caller-known exact diagnosis. The
internal one-page source runtime delegates source `Upsert` keys only to the missing-only operator
method, so stale fields or links discovered from owner scan no longer create findings through that
path.

Neither operator method exposes a database connection, source/schema registry, snapshot reader,
finding writer, raw record, typed snapshot state, lifecycle, repair, scheduler, or worker handle.

## Deliberate limits

This slice does not add or claim:

- exported PostgreSQL snapshots shared with arbitrary owner adapters;
- source `Missing` admission without a retained tombstone or explicit positive watermark;
- a public source-page transport or caller-visible source cursor;
- cross-page accumulation, background iteration, scheduling, or restart state;
- stale Index-only enumeration or orphan-link diagnosis;
- finding resolution when states converge;
- resolve/ignore commands, actor/reason audit, or additional authorization policy;
- targeted/full/shadow repair;
- retained PostgreSQL, authorization, lifecycle, GraphQL, or production-source execution evidence.

## Maintainer verification

```bash
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-index --test drift_finding_locale_key_contract
cargo test -p rustok-server index_drift_diagnosis -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test drift_snapshot_reader_postgres_test \
  -- --nocapture --test-threads=1

cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-drift-digest-producer.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-drift-snapshot-reader.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-drift-finding-locale-scope.mjs
git diff --check
```

Tests, verifiers, formatting, Cargo commands, PostgreSQL or GraphQL runs, workflows, and CI were not
executed by the implementation agent.
