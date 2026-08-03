# Index reconciliation operator runtime

Status: `source_complete_transport_and_scheduling_pending`.

## Purpose

The server publishes one guarded `IndexReconciliationOperatorRuntime` after the existing Index replay composition has frozen the complete immutable source and schema registries.

The boundary wraps three canonical PostgreSQL capabilities exported by `rustok-index`:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only failed-job inspection;
- `PostgresIndexReconciliationRecoveryStore` for audited same-job failed-to-pending recovery.

Composition performs no reconciliation database I/O, starts no task, and creates no second source catalog.

## Request-bound authority

Every operation requires an `IndexReconciliationOperatorContext` containing one non-nil tenant UUID and actor UUID.

Authorization reads the current request-scoped RBAC snapshot through `permissions_for(tenant_id, actor_id)` and requires effective `Permission::MODULES_MANAGE`.

The boundary rejects:

- a nil tenant or actor identity;
- a missing request-bound permission snapshot;
- a snapshot without `modules:manage`;
- a run request whose tenant differs from the authorized context tenant.

Run authorization compares the requested tenant before delegating to the inner reconciliation runner. The focused source regression uses a database without Index migrations and still receives `TenantMismatch`, retaining the pre-database denial boundary.

Cancellation, dead-letter inspection, and dead-letter requeue accept no caller-selected tenant. Their tenant scope is derived only from the authorized context.

Requeue also accepts no caller-selected actor. The actor written to the immutable recovery audit is always `context.actor_id()`.

Inspection and requeue authorization run before adapter or recovery-request validation and before database access. Missing authority returns `MissingRequestAuthority`; `modules:read` alone returns `Forbidden`; only an authorized call reaches the bounded crate validation surface.

## Published surface

The capability exposes only:

- bounded `run(context, IndexReconciliationRunRequest)`;
- tenant-scoped `request_cancel(context, job_id)`;
- tenant-scoped read-only `inspect_dead_letter(context, job_id)`;
- tenant/actor-bound `requeue_dead_letter(context, job_id, reason)`.

A successful dead-letter inspection returns only the bounded crate inspection result.

A successful requeue returns only the bounded crate recovery outcome: generated audit UUID, retained job UUID, and incremented retry epoch. The server does not duplicate recovery SQL, cursor reset logic, scope locking, audit insertion, or job-state mutation.

Raw `last_error_details`, tenant identity, request/cursor JSON, worker and lease fields, timestamps, SQL, database causes, and payloads remain unavailable through the server runtime.

The runtime does not expose:

- the database connection;
- source or schema registries;
- mutable source catalogs;
- scheduler, polling, takeover, or task ownership;
- worker-spawn handles;
- direct `index_jobs` or recovery-audit SQL;
- caller-selected tenant or actor identity for recovery;
- GraphQL, HTTP, CLI, MCP, or admin transport.

## Recovery ordering

`requeue_dead_letter` performs these server-boundary steps in order:

1. authorize the exact context tenant and actor with request-scoped `modules:manage`;
2. construct `IndexReconciliationRequeueRequest` from `context.tenant_id()`, the caller job UUID, `context.actor_id()`, and the explicit reason;
3. delegate to `PostgresIndexReconciliationRecoveryStore::requeue_failed`.

The reason remains owned by the crate validation contract: non-empty, trimmed, control-character-free, and no more than 512 UTF-8 bytes.

Authorization therefore wins over malformed job IDs or reasons, and unauthorized callers cannot use validation differences as a recovery oracle.

## Composition

The existing server replay composition remains the single source-freezing point:

1. selected PostgreSQL source factories are materialized;
2. the complete immutable `SharedIndexSourceRegistry` is inserted;
3. the guarded replay capability is materialized from the shared registries;
4. the guarded reconciliation operator is constructed from the same source registry, shared schema registry, and host database;
5. the operator receives a recovery store and read-only inspector over cloned host database handles, plus the canonical runner over the retained handle.

An absent source registry publishes no false reconciliation capability. A source registry without `SharedIndexSchemaRegistry` fails closed. Duplicate guarded reconciliation materialization also fails closed.

## Preserved engine behavior

This server slice changes no migration, recovery SQL, reconciliation runner, inspector query, state transition, lease, cursor, mutation, diagnostic, cancellation, or terminal-state contract.

The same-job failed-to-pending reset, retry-epoch increment, scope advisory lock, attempt/epoch fencing, and immutable actor/reason audit remain entirely owned by `rustok-index`.

## Explicitly open

- GraphQL, HTTP, CLI, MCP, admin, or other command transport;
- automatic retry, backoff, exhaustion, scheduling, or takeover discovery;
- graceful task shutdown and fleet coordination;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair modes;
- locale or partition checkpoint dimensions;
- retained PostgreSQL authorization, inspection, cancellation, restart, concurrency, and recovery evidence.

The canonical bounded retry/global scheduling and drift-diagnosis/targeted-repair roadmap items remain open. Authorized manual recovery does not establish automatic scheduling, complete drift repair, or production readiness.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
