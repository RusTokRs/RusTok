# `rustok-index` implementation recheck — explicit absence watermark

Audited baseline: `main@2197ffaf4ca47f7cf56d8014deaaab69a1dfc51d`.
The two commits after `9cfc43cf72284e16261f788070a47367613bf2e2` change only
`rustok-order`, `rustok-pages`, and their documentation/verifiers; they do not overlap this slice.
Rechecked predecessor: PR #2983 at
`cea5e0544049c0d9610b85de67f53b9c7e6a02d4`.

## Rechecked predecessor

The guarded exact-entity diagnosis change remains internally consistent at source level:

- authorization is request-bound and precedes digest request validation and dependency access;
- the accepted key is one typed `EntityKey` and must match the authorized tenant;
- composition reuses the frozen source/schema registries and publishes no scan, scheduler, repair,
  reader, writer, registry, or connection handle;
- empty owner loads remain permanent `index_drift_source_watermark_missing`;
- no GraphQL, HTTP, CLI, admin, MCP, or other transport is claimed;
- PR #2983 has no review threads, submitted reviews, or conversation comments;
- tests and retained execution evidence remain owner-owned and pending.

One guard defect was found during this recheck. The snapshot-reader verifier searched for the
misspelled marker `PostgreSIndexDriftSnapshotReader`, while the implementation correctly defines
`PostgresIndexDriftSnapshotReader`. The replacement branch corrects the verifier marker without
changing runtime behavior.

## Continued slice

The next canonical item was an explicit retained absence/tombstone watermark contract. This branch
adds a database-neutral optional registry rather than weakening the existing targeted-load result:

- `IndexSourceAbsenceWatermark` carries one exact typed key and one positive source version;
- `IndexSourceAbsenceProvider` returns `Some(watermark)`, non-authoritative `None`, or the existing
  bounded retryable/permanent `IndexSourceFailure`;
- provider names, schema sets, and schema-identity ownership are bounded and deterministic;
- materialization requires the frozen canonical replay source registry;
- every provider owner must equal the replay source owner for every exact schema;
- registry lookup performs one exact call and rejects a cross-scope result;
- no scan, ID collection, SQL, scheduling, lifecycle transition, or repair is introduced.

Existing `IndexSource::scan`, `IndexSource::load`, and every current owner adapter remain
source-compatible. The PostgreSQL drift reader is deliberately not wired in this contract-only
slice, so an empty targeted load still cannot become source `Missing`.

## Open cursor

The next implementation step is to register one production owner-retained provider and wire the
frozen absence registry into `PostgresIndexDriftSnapshotReader`. Snapshot capture must compare the
same positive absence version on both sides of the materialized read and bind it into the opaque
boundary. Missing registration, `None`, key mismatch, zero version, or a changed version must remain
fail-closed.

Diagnosis transport, discovery, automatic finding resolution, resolve/ignore commands, repair, and
retained execution evidence remain open.

## Validation ownership

Suggested commands are retained in the dated implementation plan and the explicit watermark
document. Per maintainer instruction, this implementation agent did not run tests, JavaScript
verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI.
