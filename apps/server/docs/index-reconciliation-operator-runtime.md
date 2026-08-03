# Index reconciliation operator runtime

Status: `source_complete_transport_and_recovery_work_pending`.

## Purpose

The server publishes one guarded `IndexReconciliationOperatorRuntime` after the existing Index replay composition has frozen the complete immutable source and schema registries.

The boundary wraps the canonical `PostgresIndexReconciliationRunner` already exported by `rustok-index`. It performs no reconciliation database I/O during composition, starts no task, and does not create another source catalog.

## Request-bound authority

Every operation requires an `IndexReconciliationOperatorContext` containing one non-nil tenant UUID and actor UUID.

Authorization reads the current request-scoped RBAC snapshot through `permissions_for(tenant_id, actor_id)` and requires effective `Permission::MODULES_MANAGE`.

The boundary rejects:

- a nil tenant or actor identity;
- a missing request-bound permission snapshot;
- a snapshot without `modules:manage`;
- a run request whose tenant differs from the authorized context tenant.

Run authorization compares the requested tenant before delegating to the inner reconciliation runner. The focused source regression uses a database without Index migrations and still receives `TenantMismatch`, retaining the pre-database denial boundary.

Cancellation accepts only a job UUID from the caller. The tenant passed to the reconciliation runner is always derived from the authorized context; there is no separate caller-selected tenant parameter.

## Published surface

The capability exposes only:

- bounded `run(context, IndexReconciliationRunRequest)`;
- tenant-scoped `request_cancel(context, job_id)`.

It does not expose:

- the database connection;
- source or schema registries;
- mutable source catalogs;
- scheduler, polling, takeover, or task ownership;
- worker-spawn handles;
- direct `index_jobs` SQL;
- dead-letter inspection or requeue;
- GraphQL, HTTP, CLI, MCP, or admin transport.

## Composition

The existing server replay composition remains the single source-freezing point:

1. selected PostgreSQL source factories are materialized;
2. the complete immutable `SharedIndexSourceRegistry` is inserted;
3. the guarded replay capability is materialized from the shared registries;
4. the guarded reconciliation operator is constructed from the same source registry, shared schema registry, and host database.

An absent source registry publishes no false reconciliation capability. A source registry without `SharedIndexSchemaRegistry` fails closed. Duplicate guarded reconciliation materialization also fails closed.

The replay retry transition store and replay failed-scope admission merged separately. They do not alter this reconciliation operator and are not reused as reconciliation scheduling or dead-letter APIs.

## Preserved engine behavior

This server slice changes no reconciliation runner, migration, SQL, lease, cursor, mutation, diagnostic, cancellation, or terminal-state contract.

It preserves the existing bounded page/pass execution, heartbeat, yield, cancellation, attempt fencing, inbox deduplication, source-version monotonicity, and bounded machine-readable failure diagnostics.

## Explicitly open

- reconciliation failed-scope admission;
- bounded reconciliation dead-letter inspection;
- authorized requeue with actor/reason audit and retry-epoch semantics;
- automatic scheduling or takeover discovery;
- graceful task shutdown and fleet coordination;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair modes;
- locale or partition checkpoint dimensions;
- retained PostgreSQL authorization, cancellation, restart, concurrency, and recovery evidence;
- command and transport publication.

The canonical M6 drift-diagnosis and targeted-repair roadmap item remains open. Runtime publication alone does not establish complete drift repair or production readiness.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
