# Index reconciliation operator runtime

Status: `source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes guarded Index reconciliation capabilities after replay composition freezes the immutable source and schema registries.

The reconciliation boundary wraps:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only dead-letter inspection;
- `PostgresIndexDriftFindingInspector` for bounded read-only finding inspection;
- `PostgresIndexReconciliationRecoveryStore` for audited same-job recovery.

The same source-freezing point now also publishes a separate `IndexDriftDiagnosisOperatorRuntime`. Keeping exact-entity diagnosis in a sibling capability prevents the runner/recovery runtime from owning a snapshot reader or finding writer while preserving the same request-bound authority and composition order.

The registry-freezing composition also publishes the separate module-owned reconciliation work registration. Neither guarded operator exposes or owns that scheduler.

## Request-bound authority

Every operation requires a non-nil tenant/actor `IndexReconciliationOperatorContext`, a current request-scoped RBAC snapshot, and effective `Permission::MODULES_MANAGE`.

Run rejects a tenant mismatch before runner delegation. Cancellation, dead-letter inspection, drift-finding inspection, and requeue accept no caller-selected tenant. Requeue accepts no caller-selected actor; the audit actor is always `context.actor_id()`.

Exact-entity diagnosis accepts one typed `EntityKey`. Its embedded tenant must equal `context.tenant_id()`. Authorization runs before `IndexDriftDigestRequest` validation, snapshot capture, digest production, or finding persistence, so malformed or cross-tenant keys cannot be used as a source or storage oracle.

Inspection, diagnosis, and requeue authorize before adapter or request validation and before database access.

## Published surface

`IndexReconciliationOperatorRuntime` exposes only:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `inspect_drift_finding(context, finding_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

`IndexDriftDiagnosisOperatorRuntime` exposes only:

- `diagnose_entity(context, key)`.

Diagnosis composes `PostgresIndexDriftSnapshotReader`, `IndexDriftDigestProducer`, and `PostgresIndexDriftFindingWriter`. A consistent pair returns the bounded digest. A mismatch returns only source/materialized digests and the finding receipt. The operator does not return raw records, fields, links, SQL, database causes, source payloads, credentials, or transaction details.

Drift-finding inspection returns only the bounded crate value: finding UUID and key, check name, severity, typed scope, and optional expected/actual digests. It does not return tenant identity, raw finding details, detection timestamps, closure state, SQL, or database causes.

The runtimes expose no database connection, source/schema registry, scheduler handle, worker-spawn handle, snapshot reader, finding writer, raw failure details, direct SQL, or transport.

## Composition and scheduling

The server replay composition remains the single source-freezing point:

1. PostgreSQL source factories are materialized;
2. `SharedIndexSourceRegistry` is frozen;
3. replay dry-run/runtime and due-reconciliation module-work registration are published;
4. the guarded reconciliation operator is built from the immutable source/schema registries and host database;
5. the guarded exact-entity diagnosis operator is built from those same registries and database;
6. both capabilities are inserted into `ModuleRuntimeExtensions` before host context publication.

Composition performs no reconciliation or diagnosis SQL and starts no task. PostgreSQL backend enforcement occurs inside the snapshot reader before source or materialized reads. The existing generic server module-work bootstrap later owns the polling loop and shared shutdown. Diagnosis is explicit, request-bound, and never scheduled by this slice.

## Exact-entity diagnosis limits

The command accepts exactly one key and invokes no source scan. Empty targeted owner loads remain permanent `index_drift_source_watermark_missing`; the operator does not reinterpret unproven absence as authoritative `Missing`.

The command can create, refresh, reopen, or suppress the deterministic digest-mismatch finding through the existing writer lifecycle. It does not resolve a finding when states converge, ignore findings, choose repair policy, or execute repair.

## Explicitly open

- GraphQL, HTTP, CLI, MCP, native admin, or other diagnosis transport;
- retained PostgreSQL authorization, diagnosis, finding-lifecycle, and scheduler execution evidence;
- explicit retained absence/tombstone watermark support for empty targeted loads;
- bounded entity discovery, missing/stale enumeration, and orphan-link diagnosis;
- finding resolution or ignore transitions with actor/reason audit;
- targeted/full/shadow repair admission, execution, audit, and evidence;
- operator-visible scheduler health and metrics;
- per-source retry policy, jitter, and dynamic configuration;
- locale or partition checkpoint dimensions.

The canonical bounded retry/global scheduling item remains open pending owner-retained production and multi-host evidence. Exact-entity digest diagnosis is source complete; broader diagnosis and all repair remain open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
