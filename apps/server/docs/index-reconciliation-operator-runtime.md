# Index reconciliation operator runtime

Status: implementation retained, not run.

## Purpose

The server publishes one guarded `IndexReconciliationOperatorRuntime` after the existing Index replay composition has frozen the complete immutable source and schema registries.

The boundary wraps the canonical `PostgresIndexReconciliationRunner` directly from current `main`. It does not depend on an unmerged Index runtime abstraction and performs no database I/O during composition.

## Request-bound authority

Every operation requires an `IndexReconciliationOperatorContext` containing one non-nil tenant UUID and actor UUID.

Authorization is evaluated from the current request-scoped RBAC snapshot through `permissions_for(tenant_id, actor_id)`.

The boundary requires exactly `Permission::MODULES_MANAGE` and rejects:

- a nil tenant or actor identity;
- a missing request-bound permission snapshot;
- a snapshot without `modules:manage`;
- a run request whose tenant differs from the authorized context tenant.

The tenant comparison occurs before the inner reconciliation runner and therefore before database access.

Cancellation does not accept a caller-supplied tenant separate from the context. It always calls the runner with the authorized context tenant and the requested job UUID.

## Published surface

The server capability exposes only:

- bounded `run(IndexReconciliationRunRequest)`;
- tenant-scoped `request_cancel(job_id)`.

It does not expose:

- the database connection;
- source or schema registries;
- mutable source catalogs;
- scheduler or poller ownership;
- task handles or worker spawning;
- direct `index_jobs` SQL;
- GraphQL, HTTP, CLI, MCP, or admin transport.

## Composition

The existing server replay composition remains the single source-freezing point.

After it materializes PostgreSQL source factories and inserts the immutable `SharedIndexSourceRegistry`, it:

1. materializes the existing guarded replay capability;
2. requires the already-published `SharedIndexSchemaRegistry` when sources exist;
3. constructs `PostgresIndexReconciliationRunner` from the host database and exact shared registries;
4. inserts one guarded reconciliation operator into `ModuleRuntimeExtensions`.

An absent source registry publishes no false reconciliation capability. A source registry without its shared schema registry fails closed. Duplicate guarded reconciliation materialization also fails closed.

## Preserved engine behavior

This server slice does not change the reconciliation runner, migrations, SQL, leases, cursor shape, event identity, mutations, diagnostics, or terminal state machine.

The published boundary preserves existing bounded page/pass execution, heartbeat, yield, cancellation, attempt fencing, inbox deduplication, source-version monotonicity, and bounded failure diagnostics.

## Scope boundaries

This slice does not add:

- automatic scheduling or takeover discovery;
- retry/backoff or dead-letter requeue;
- graceful task shutdown;
- source/index digest comparison;
- orphan cleanup;
- targeted, full, or shadow repair modes;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer validation

```bash
cargo test -p rustok-server index_reconciliation_operator -- --nocapture
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-server-reconciliation-guard.mjs
```

These commands were not run by the implementation agent.
