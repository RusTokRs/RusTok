# Index reconciliation operator runtime

Status: `diagnosis_graphql_transport_source_complete_owner_execution_pending`.

## Purpose

The server publishes guarded Index reconciliation and exact-entity diagnosis capabilities after
replay composition freezes the immutable source and schema registries.

`IndexReconciliationOperatorRuntime` privately wraps:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only dead-letter inspection;
- `PostgresIndexDriftFindingInspector` for bounded read-only finding inspection;
- `PostgresIndexReconciliationRecoveryStore` for audited same-job recovery.

The sibling `IndexDriftDiagnosisOperatorRuntime` privately wraps:

- `PostgresIndexDriftSnapshotReader`;
- `IndexDriftDigestProducer`;
- `PostgresIndexDriftFindingWriter`.

Keeping diagnosis separate prevents reconciliation run/recovery from owning a snapshot reader or
finding writer. The separate module-work registration and host scheduler remain outside both
operator surfaces.

## Request-bound authority

Every operation requires a non-nil tenant/actor `IndexReconciliationOperatorContext`, a current
request-scoped RBAC snapshot, and effective `Permission::MODULES_MANAGE`.

Run rejects tenant mismatch before runner delegation. Cancellation, inspections, and requeue accept
no caller-selected tenant. Requeue accepts no caller-selected actor; its audit actor is always
`context.actor_id()`.

`diagnose_entity(context, key)` accepts one typed `EntityKey`. Its tenant must equal the context
tenant. Authorization runs before `IndexDriftDigestRequest` validation, owner-source access,
materialized reads, digest production, or finding persistence.

## Published operator surface

`IndexReconciliationOperatorRuntime` exposes only:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `inspect_drift_finding(context, finding_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

`IndexDriftDiagnosisOperatorRuntime` exposes only:

- `diagnose_entity(context, key)`.

Diagnosis returns only the bounded digest outcome. It exposes no raw owner record, fields, links,
SQL, database cause, credential, transaction, source registry, absence registry, snapshot reader,
finding writer, scheduler, or repair handle.

## GraphQL diagnosis transport

The root GraphQL mutation now includes one server-owned exact-entity operation:

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
They expose no raw adapter or database cause. The mutation owns no batch, scan, discovery, finding
lifecycle, scheduler, or repair operation.

## Composition

The server replay composition remains the single source-freezing point:

1. selected PostgreSQL source factories construct ordinary replay sources and optional owner
   absence providers without executing SQL;
2. `SharedIndexSourceRegistry` is frozen;
3. replay runtime and reconciliation work registration are published;
4. the guarded reconciliation operator is constructed;
5. diagnosis materializes the optional `SharedIndexSourceAbsenceRegistry`, verifying owner parity
   against the frozen replay registry;
6. the diagnosis reader receives the same immutable source/schema registries and, when present, the
   private absence registry;
7. both guarded operators are inserted before host-context publication;
8. GraphQL schema construction receives the frozen `ModuleRuntimeExtensions` and mounts the bounded
   diagnosis mutation without reconstructing any adapter.

Composition performs no reconciliation or diagnosis SQL and starts no task. The generic server
module-work bootstrap owns polling and shared shutdown. Exact-entity diagnosis remains explicit and
is never scheduled by this slice.

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

- reconciliation run/cancel/inspection/requeue transports;
- retained GraphQL authorization, Product absence, diagnosis, finding-lifecycle, scheduler, and
  multi-host execution evidence;
- bounded entity discovery, missing/stale enumeration, and orphan-link diagnosis;
- finding resolution or ignore transitions with actor/reason audit;
- targeted/full/shadow repair admission, execution, audit, and evidence;
- operator-visible scheduler health and metrics;
- per-source retry policy, jitter, and dynamic configuration;
- locale or partition checkpoint dimensions.

Exact-entity digest diagnosis, Product locale-absence fencing, and the bounded GraphQL diagnosis
transport are source complete. Broader diagnosis, all repair, reconciliation transports, and retained
execution evidence remain open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database or GraphQL scenarios, workflows, and
CI are maintainer-run and were not executed by the implementation agent.
