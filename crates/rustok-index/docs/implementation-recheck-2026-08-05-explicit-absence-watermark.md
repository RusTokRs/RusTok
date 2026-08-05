# `rustok-index` implementation recheck — explicit absence watermark

Audited baseline: `main@368c79b78549e97a68120358021552b2552b800c`.
Latest default-branch delta checked through
`main@4fe2643c0d3f3e7e3c0e5e2ccf9347184f347395`.
The fifteen main commits after this branch merge base touch Commerce diagnostics, Forum module-owned
GraphQL transports, Inventory/Order diagnostics, and Pages/Page Builder evidence. They do not
overlap `rustok-index`, Product Index composition, the server Index GraphQL root, diagnosis
composition, or Index guards changed by this branch.
Rechecked predecessor: PR #2983 at
`cea5e0544049c0d9610b85de67f53b9c7e6a02d4`.

## Rechecked predecessor

The guarded exact-entity diagnosis change remains internally consistent at source level:

- authorization is request-bound and precedes digest request validation and dependency access;
- the accepted key is one typed `EntityKey` and must match the authorized tenant;
- composition reuses the frozen source/schema registries and publishes no scheduler, repair,
  reader, writer, registry, or connection handle;
- the predecessor itself claimed no GraphQL, HTTP, CLI, admin, MCP, or other transport;
- PR #2983 had no review threads, submitted reviews, or conversation comments;
- tests and retained execution evidence remain owner-owned and pending.

One guard defect was found during the recheck. The snapshot-reader verifier searched for the
misspelled marker `PostgreSIndexDriftSnapshotReader`, while the implementation correctly defines
`PostgresIndexDriftSnapshotReader`. This branch corrects that verifier marker. A separate inherited
reconciliation verifier typo was corrected without changing runner behavior.

## Explicit absence contract

The branch retains the database-neutral optional registry without weakening ordinary targeted load:

- `IndexSourceAbsenceWatermark` carries one exact typed key and one positive source version;
- `IndexSourceAbsenceProvider` returns `Some(watermark)`, non-authoritative `None`, or one existing
  bounded retryable/permanent `IndexSourceFailure`;
- provider names, schema sets, and schema-identity ownership are bounded and deterministic;
- materialization requires the frozen canonical replay source registry;
- every provider owner must equal the replay source owner for every exact schema;
- registry lookup performs one exact call and rejects cross-scope evidence;
- existing `IndexSource::scan` and `IndexSource::load` implementations remain source-compatible.

## Production continuation

The selected Product bridge registers `ProductLocaleAbsenceProvider` as
`product-locale-absence-postgres` for Product schema versions 1 and 2. It returns positive
`products.index_revision` only when the live Product exists, the exact translation locale is absent,
and no exact Product tombstone owns that locale.

This is a truthful source high-watermark because Product translation insert, delete, and reassignment
advance the same revision. Hard deletes remain ordinary retained `Delete` mutations rather than
being reclassified as `Missing`.

Guarded diagnosis materializes the optional absence registry after the canonical source registry
and attaches it privately to `PostgresIndexDriftSnapshotReader`. For an empty load the reader requires
an exact watermark, opens the existing read-only repeatable-read materialized snapshot, and reloads
both ordinary source state and the watermark before accepting the pair.

The reader compares the typed `Missing` state plus positive absence version. A changed version, a
newly appearing source mutation, or loss of proof after the first positive observation returns
retryable `index_drift_source_changed_during_capture`. The absence version is domain-tagged into the
opaque boundary only for source `Missing`; existing Upsert/Delete boundary derivation remains
unchanged.

Missing registration, provider `None`, key mismatch, zero version, and malformed evidence remain
fail-closed. An empty targeted load alone still returns `index_drift_source_watermark_missing`.

## Source-ready PostgreSQL continuation

`crates/rustok-distribution/tests/product_locale_absence_postgres.rs` retains the real-migration
Product locale absence scenario without replacing either production adapter:

- it applies the complete Product migration list and every Index migration inside one isolated
  PostgreSQL schema;
- it builds selected runtime extensions from the real `IndexModule` and `ProductModule`;
- it materializes the production Product replay source, owner-bound absence registry, and
  `PostgresIndexDriftSnapshotReader` through their public composition functions;
- a stable French-locale absence must return exact source/materialized `Missing` states with a
  bounded `pg:` boundary;
- a second scenario blocks only the exact `index_entities` materialized read, waits through
  `pg_stat_activity`, inserts the requested Product translation through a separate connection, and
  requires retryable `index_drift_source_changed_during_capture` after the real second owner read.

The harness uses separate one-connection pools for owner source reads, snapshot capture, locking,
translation writing, and observation. It copies neither the Product provider SQL nor a fake
`IndexSourceAbsenceProvider`, and it adds no test callback to production code.

The harness is `source_ready_owner_execution_pending`. Its presence is not retained execution
evidence; the repository owner must run and admit the PostgreSQL output.

## Bounded GraphQL transport continuation

`apps/server/src/graphql/index_drift_diagnosis.rs` mounts one root mutation over the existing
guarded diagnosis operator:

- `IndexDriftDiagnosisInput` contains only string module/entity/version/entity-id fields and an
  optional locale; tenant and actor are absent;
- tenant and actor are derived from authenticated GraphQL request context;
- the task-local effective `modules:manage` snapshot is checked before parsing any untrusted
  identifier, schema version, UUID, or locale;
- every string is bounded before domain parsing;
- the resulting key is exactly one tenant-bound `EntityKey`;
- the resolver delegates once to `IndexDriftDiagnosisOperatorRuntime::diagnose_entity`, which
  repeats the same request-bound authorization before dependency access;
- the payload exposes only consistent/mismatch status, bounded SHA-256 digests, and finding receipt
  metadata;
- dependency failures expose only fixed public codes, retryability, and an existing bounded
  dependency code.

The transport performs no SeaORM query and owns no database connection, source/schema/absence
registry, batch, scan, scheduler, finding lifecycle, or repair capability. A source guard retains the
schema mount, authorization-before-parsing order, exact-key construction, bounded output, and open
execution status.

## One-page source candidate continuation

`IndexDriftSourcePageDiagnosisRuntime` is composed only after the frozen
`SharedIndexSourceRegistry` and guarded exact diagnosis runtime exist.

Its one method:

- derives tenant from `IndexReconciliationOperatorContext` and accepts no caller-selected tenant;
- checks the current request-local effective `modules:manage` snapshot before page-limit validation;
- allows one `SchemaRef`, one optional server-held `IndexSourceCursor`, and a limit in `1..=32`;
- performs exactly one validated `IndexSource::scan` call;
- skips retained source `Delete` mutations;
- sequentially delegates every source `Upsert` candidate to exact diagnosis;
- stops on the first source or exact-diagnosis failure;
- returns only page counts, bounded finding receipts, and the server-held next cursor;
- owns no loop, checkpoint store, job, scheduler, task, transport, lifecycle, or repair handle.

The current exact digest outcome intentionally hides mismatch state shape. The page runtime can
therefore identify and persist general exact mismatches for source-present candidates, but it cannot
truthfully claim missing-only classification. It is not mounted into GraphQL, HTTP, CLI, MCP, or
native admin, and it copies no source entity identifier or payload into its outcome.

## Open cursor

The next implementation step is a database-neutral missing-only outcome over one already-captured
`IndexDriftSnapshotPair`. It must preserve the general exact producer behavior while recording only
source `Upsert` plus materialized `Missing`, returning bounded non-candidate outcomes for all other
state combinations without exposing raw states.

Product locale PostgreSQL harness execution, GraphQL execution evidence, source-page transport,
cursor persistence, multi-page lifecycle, stale Index-only discovery, orphan-link diagnosis,
automatic finding resolution, resolve/ignore commands, and repair remain open.

## Validation ownership

Suggested commands are retained in the dated implementation plan and the explicit watermark,
harness, GraphQL transport, and source-page diagnosis documents. Per maintainer instruction, this
implementation agent did not run tests, JavaScript verifiers, formatting, Cargo checks, PostgreSQL or
GraphQL scenarios, workflows, or CI.
