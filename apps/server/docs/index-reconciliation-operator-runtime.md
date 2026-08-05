# Index reconciliation operator runtime

Status: `missing_only_source_page_source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes guarded Index reconciliation, exact-entity diagnosis, and one-page
missing-entity diagnosis capabilities after replay composition freezes the immutable source and
schema registries.

`IndexReconciliationOperatorRuntime` privately wraps:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only dead-letter inspection;
- `PostgresIndexDriftFindingInspector` for bounded read-only finding inspection;
- `PostgresIndexReconciliationRecoveryStore` for audited same-job recovery.

The sibling `IndexDriftDiagnosisOperatorRuntime` privately wraps:

- `PostgresIndexDriftSnapshotReader`;
- `IndexDriftDigestProducer`;
- `PostgresIndexDriftFindingWriter`.

`IndexDriftSourcePageDiagnosisRuntime` privately composes:

- the frozen `SharedIndexSourceRegistry` for one bounded owner-source page;
- the guarded `IndexDriftDiagnosisOperatorRuntime` for sequential missing-only candidate diagnosis.

Keeping these capabilities separate prevents reconciliation run/recovery from owning a snapshot
reader or finding writer, and prevents source-page diagnosis from owning a loop, checkpoint store,
scheduler, or task. The separate module-work registration and host scheduler remain outside all
three operator surfaces.

## Request-bound authority

Every operation requires a non-nil tenant/actor `IndexReconciliationOperatorContext`, a current
request-scoped RBAC snapshot, and effective `Permission::MODULES_MANAGE`.

Run rejects tenant mismatch before runner delegation. Cancellation, inspections, and requeue accept
no caller-selected tenant. Requeue accepts no caller-selected actor; its audit actor is always
`context.actor_id()`.

`diagnose_entity(context, key)` and `diagnose_missing_entity_candidate(context, key)` each accept one
typed `EntityKey`. Its tenant must equal the context tenant. Authorization runs before
`IndexDriftDigestRequest` validation, owner-source access, materialized reads, typed-state
classification, digest production, or finding persistence.

`diagnose_source_page(context, schema, cursor, limit)` derives tenant from the operator context. It
checks the same current request-local permission snapshot before validating the `1..=32` limit or
constructing `IndexSourceScanRequest`. The maximum page size of 32 is intentionally stricter than the
generic source-scan contract. Every source `Upsert` is then delegated to
`diagnose_missing_entity_candidate`, which repeats authorization before exact dependency access.

## Published operator surface

`IndexReconciliationOperatorRuntime` exposes only:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `inspect_drift_finding(context, finding_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

`IndexDriftDiagnosisOperatorRuntime` exposes only:

- `diagnose_entity(context, key)` for general caller-known exact diagnosis;
- `diagnose_missing_entity_candidate(context, key)` for missing-only internal discovery.

`IndexDriftSourcePageDiagnosisRuntime` exposes only:

- `diagnose_source_page(context, schema, cursor, limit)`.

General exact diagnosis returns the bounded digest outcome. Missing-only exact diagnosis returns only
`NotCandidate` or `MissingRecorded`. Source-page diagnosis returns current-page counts, bounded
missing finding receipts, and one server-held continuation cursor.

No capability exposes raw owner records, indexed records, fields, links, SQL, database causes,
credentials, transactions, registry handles, snapshot states, scheduler handles, or repair handles.

The source-page runtime skips retained source `Delete` mutations and sequentially diagnoses only
source `Upsert` candidates. Materialized `Upsert` or `Delete`, including stale field/link/version
mismatches, returns `NotCandidate` and does not call the finding recorder through this path.

## GraphQL diagnosis transport

The root GraphQL mutation includes one server-owned exact-entity operation:

- `diagnoseIndexEntity(input: IndexDriftDiagnosisInput!): IndexDriftDiagnosisPayload!`.

The input contains string forms of module, entity, schema version, entity UUID, and optional locale.
It contains no tenant or actor field. The resolver derives both identities from authenticated request
context and checks the task-local effective `modules:manage` snapshot before parsing any untrusted
identifier, version, UUID, or locale.

After bounded parsing, the mutation delegates exactly once to `diagnose_entity(context, key)`. The
operator repeats authorization before source or database access. This defense-in-depth check uses the
same request-local permission snapshot and creates no second authority cache or database permission
lookup.

The payload exposes only:

- `CONSISTENT` plus one digest; or
- `MISMATCH_RECORDED` plus source/materialized digests and bounded finding receipt metadata.

Dependency errors expose fixed GraphQL codes, retryability, and the existing bounded dependency code.
They expose no raw adapter or database cause.

The missing-only operator method, `IndexDriftSourcePageDiagnosisRuntime`, its source cursor, page
counters, and source scan capability are not attached to GraphQL by this slice.

## Composition

The server replay composition remains the single source-freezing point:

1. selected PostgreSQL source factories construct ordinary replay sources and optional owner
   absence providers without executing SQL;
2. `SharedIndexSourceRegistry` is frozen;
3. replay runtime and reconciliation work registration are published;
4. the guarded reconciliation operator is constructed;
5. exact diagnosis materializes the optional `SharedIndexSourceAbsenceRegistry`, verifying owner
   parity against the frozen replay registry;
6. the diagnosis reader receives the same immutable source/schema registries and, when present, the
   private absence registry;
7. general and missing-only exact diagnosis methods are inserted through one guarded runtime;
8. one-page missing-entity diagnosis is inserted only after both the frozen source registry and exact
   diagnosis runtime exist;
9. GraphQL schema construction receives the frozen `ModuleRuntimeExtensions` and mounts only the
   bounded caller-known exact-entity diagnosis mutation.

Composition performs no reconciliation or diagnosis SQL and starts no task. The generic server
module-work bootstrap owns polling and shared shutdown. Exact-entity and source-page diagnosis remain
explicit and are never scheduled by this slice.

## Explicit Product locale absence

For Product v1/v2, the selected distribution may return an exact positive
`products.index_revision` when the Product exists but the requested translation locale and exact
locale tombstone are both absent.

An empty ordinary targeted load is accepted as source `Missing` only with that explicit watermark.
The reader reloads the ordinary source and the same positive watermark while its PostgreSQL
materialized snapshot remains open. A changed owner state or version returns retryable
`index_drift_source_changed_during_capture`.

Missing provider registration, provider `None`, malformed evidence, or unproven absence remains
permanent `index_drift_source_watermark_missing` or another bounded contract failure. Retained hard
deletes remain ordinary source `Delete` values.

## Explicitly open

- a GraphQL, HTTP, CLI, MCP, or native-admin source-page transport;
- a sealed request-bound continuation envelope for the source cursor;
- cursor persistence, multi-page accumulation, background iteration, scheduling, or restart state;
- reconciliation run/cancel/inspection/requeue transports;
- retained GraphQL authorization, Product absence, diagnosis, finding-lifecycle, scheduler, and
  multi-host execution evidence;
- bounded Index-only stale enumeration and orphan-link diagnosis;
- finding resolution or ignore transitions with actor/reason audit;
- targeted/full/shadow repair admission, execution, audit, and evidence;
- operator-visible scheduler health and metrics;
- per-source retry policy, jitter, and dynamic configuration;
- locale or partition checkpoint dimensions.

Exact-entity digest diagnosis, Product locale-absence fencing, bounded exact GraphQL transport, and
one-page internal missing-entity diagnosis are source complete. Source-page transport, broader
diagnosis, all repair, reconciliation transports, and retained execution evidence remain open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database or GraphQL scenarios, workflows, and
CI are maintainer-run and were not executed by the implementation agent.
