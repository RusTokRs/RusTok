# Index reconciliation operator runtime

Status: `source_complete_transport_and_recovery_work_pending`.

## Purpose

The server publishes one guarded `IndexReconciliationOperatorRuntime` after the existing Index replay composition has frozen the complete immutable source and schema registries.

The boundary wraps both canonical PostgreSQL capabilities exported by `rustok-index`:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only failed-job inspection.

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

Cancellation and dead-letter inspection accept only a job UUID from the caller. The tenant delegated to the runner or inspector is always derived from the authorized context; neither operation accepts a separate caller-selected tenant.

Inspection authorization runs before adapter validation or database access. Without request authority it returns `MissingRequestAuthority`; `modules:read` alone returns `Forbidden`; an authorized call then delegates to the bounded crate inspector.

## Published surface

The capability exposes only:

- bounded `run(context, IndexReconciliationRunRequest)`;
- tenant-scoped `request_cancel(context, job_id)`;
- tenant-scoped read-only `inspect_dead_letter(context, job_id)`.

A successful dead-letter inspection returns only:

- failed job UUID;
- positive durable attempt count;
- optional bounded `last_error_code`;
- bounded dependency code;
- retryability.

Raw `last_error_details`, tenant identity, request/cursor JSON, worker and lease fields, timestamps, SQL, database causes, and payloads remain unavailable through the server runtime.

The runtime does not expose:

- the database connection;
- source or schema registries;
- mutable source catalogs;
- scheduler, polling, takeover, or task ownership;
- worker-spawn handles;
- direct `index_jobs` SQL;
- retry, requeue, retry-epoch reset, or failed-row mutation;
- GraphQL, HTTP, CLI, MCP, or admin transport.

## Composition

The existing server replay composition remains the single source-freezing point:

1. selected PostgreSQL source factories are materialized;
2. the complete immutable `SharedIndexSourceRegistry` is inserted;
3. the guarded replay capability is materialized from the shared registries;
4. the guarded reconciliation operator is constructed from the same source registry, shared schema registry, and host database;
5. the same operator receives a read-only dead-letter inspector over a clone of the host database handle.

An absent source registry publishes no false reconciliation capability. A source registry without `SharedIndexSchemaRegistry` fails closed. Duplicate guarded reconciliation materialization also fails closed.

The replay retry transition store, replay failed-scope admission, and reconciliation failed-scope admission remain independent engine contracts. They are not reused as server scheduling or recovery APIs.

## Preserved engine behavior

This server slice changes no reconciliation runner, inspector SQL, migration, state transition, lease, cursor, mutation, diagnostic, cancellation, or terminal-state contract.

It preserves the existing bounded page/pass execution, heartbeat, yield, cancellation, attempt fencing, inbox deduplication, source-version monotonicity, failed-scope admission, strict dead-letter diagnostic decoding, and bounded machine-readable results.

## Explicitly open

- GraphQL, HTTP, CLI, MCP, or admin inspection transport;
- authorized requeue with actor/reason audit and retry-epoch semantics;
- automatic scheduling or takeover discovery;
- graceful task shutdown and fleet coordination;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair modes;
- locale or partition checkpoint dimensions;
- retained PostgreSQL authorization, inspection, cancellation, restart, concurrency, and recovery evidence;
- command and recovery publication.

The canonical M6 drift-diagnosis and targeted-repair roadmap item remains open. Authorized read-only inspection does not establish recovery, complete drift repair, or production readiness.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
