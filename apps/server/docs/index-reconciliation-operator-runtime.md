# Index reconciliation operator runtime

Status: implementation retained, not run.

## Purpose

The server publishes guarded Index reconciliation capabilities after the existing replay composition has frozen the complete immutable source and schema registries.

The boundary wraps the canonical `PostgresIndexReconciliationRunner` and the bounded read-only `PostgresIndexReconciliationDeadLetterInspector`. Composition performs no reconciliation database I/O and starts no worker.

## Request-bound authority

Every operation requires an `IndexReconciliationOperatorContext` containing one non-nil tenant UUID and actor UUID.

Authorization is evaluated from the current request-scoped RBAC snapshot through `permissions_for(tenant_id, actor_id)`. Run, cancellation, and dead-letter inspection require `Permission::MODULES_MANAGE`.

The boundary rejects:

- a nil tenant or actor identity;
- a missing request-bound permission snapshot;
- a snapshot without `modules:manage`;
- a run request whose tenant differs from the authorized context tenant.

Run tenant comparison and dead-letter authorization occur before the inner PostgreSQL adapters and therefore before database access.

Cancellation and inspection do not accept a caller-supplied tenant separate from the context. Both derive tenant scope only from the authorized `IndexReconciliationOperatorContext`.

## Published surface

`IndexReconciliationOperatorRuntime` exposes only:

- bounded `run(IndexReconciliationRunRequest)`;
- tenant-scoped `request_cancel(job_id)`.

`IndexReconciliationDeadLetterOperatorRuntime` exposes only:

- tenant-scoped read-only `inspect_dead_letter(job_id)`.

The inspection returns the bounded Index contract: failed job UUID, positive attempt count, optional stable error code, bounded dependency code, and retryable flag. It does not return raw diagnostic JSON, tenant/schema request data, cursor state, lease/worker fields, timestamps, SQL, database causes, transport details, or stack text.

The server capabilities do not expose the database connection, source or schema registries, mutable catalogs, scheduler ownership, task handles, worker spawning, direct `index_jobs` SQL, or GraphQL/HTTP/CLI/admin transport.

## Composition

The existing server replay composition remains the single source-freezing point.

After it materializes PostgreSQL source factories and inserts the immutable `SharedIndexSourceRegistry`, it:

1. materializes the existing guarded replay capability;
2. constructs and publishes the guarded reconciliation run/cancel operator from the same source registry, shared schema registry, and host database;
3. only when that guarded reconciliation operator exists, publishes the dead-letter inspection operator over the same host database.

An absent source registry publishes no false reconciliation or dead-letter capability. A source registry without its shared schema registry fails closed. Duplicate guarded materialization also fails closed.

The original replay/reconciliation composition is retained byte-for-byte in `index_replay_runtime_composition_base.rs`; the public composition module adds only the authorized dead-letter publication layer.

## Preserved engine behavior

This server slice does not change Index migrations, reconciliation SQL, leases, cursor shape, event identity, mutation persistence, failure diagnostics, admission precedence, or terminal state transitions.

It remains read-only for inspection and does not add audit records, retry-epoch mutation, requeue, or automatic scheduling.

## Scope boundaries

This slice does not add:

- GraphQL, HTTP, CLI, MCP, or admin transport;
- actor/reason audit records;
- manual dead-letter requeue or retry-epoch reset;
- automatic retry/backoff/exhaustion or host scheduling;
- graceful task shutdown;
- source/index digest comparison or orphan cleanup;
- targeted, full, or shadow repair modes;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer validation

```bash
cargo test -p rustok-server index_reconciliation -- --nocapture
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-server-reconciliation-guard.mjs
```

These commands were not run by the implementation agent.
