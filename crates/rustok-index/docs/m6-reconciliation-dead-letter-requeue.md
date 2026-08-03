# M6 reconciliation dead-letter requeue

Status: `source_complete_authorized_server_composition_transport_pending`.

## Purpose

`PostgresIndexReconciliationRecoveryStore` provides one explicit engine-level recovery operation for an exact terminal failed reconciliation job.

The operation preserves the existing job UUID. It moves the same failed row back to `pending`, resets bounded execution state, increments a durable retry epoch, and appends an immutable actor/reason audit record in the same database transaction.

The server publishes this operation only through the request-bound `IndexReconciliationOperatorRuntime`; no transport is added.

## Request and scope contract

`IndexReconciliationRequeueRequest` requires non-nil tenant, failed-job, and actor UUIDs plus one explicit trimmed, control-character-free reason of at most 512 UTF-8 bytes.

The store resolves the exact schema-scoped reconciliation job and acquires the same PostgreSQL transaction-scoped advisory lock used by normal reconciliation admission:

```text
reconcile␟tenant␟module␟entity␟schema-version
```

It then rereads under the lock. Missing jobs return `NotFound`; non-failed jobs return `NotFailed`; concurrent state or epoch changes fail closed.

## Atomic reset and immutable audit

One transaction:

1. increments `retry_epoch`;
2. changes `failed -> pending`;
3. resets `attempt_count` to zero;
4. installs a fresh `index_reconciliation_cursor_v1` cursor;
5. clears lease, heartbeat, cancellation, error, and completion fields;
6. sets immediate availability;
7. appends tenant/job/actor/action/reason/prior-attempt/new-epoch audit evidence.

The audit insert and job reset commit or roll back together. Database triggers reject audit update/delete, and `(tenant_id, job_id, retry_epoch)` is unique.

## Authorized server composition

`IndexReconciliationOperatorRuntime::requeue_dead_letter(context, job_id, reason)` accepts no tenant or actor argument.

It authorizes the request-bound context, requires effective `Permission::MODULES_MANAGE`, derives tenant and actor from the context, constructs the bounded request, and delegates to the recovery store. Authorization occurs before job/reason validation and database access.

GraphQL, HTTP, CLI, MCP, native admin, and other command transports remain open.

## Scheduler interaction

The module-owned host scheduler is additive and unchanged by manual recovery. Requeue makes the same job immediately pending in a new retry epoch; generic due discovery can then submit it to the canonical runner. Actual claim, attempt-one fencing, cancellation, retry, exhaustion, and terminal state remain runner-owned.

Automatic retry creates no recovery audit and never increments the manual retry epoch.

## Explicitly open

- command transport mapping;
- retained PostgreSQL concurrency, authorization, recovery, due scheduling, restart, and multi-host evidence;
- operator-visible scheduler health and metrics;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair admission;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical bounded retry/global scheduling item remains open pending owner-retained production and multi-host evidence. The drift-diagnosis/targeted-repair item remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, migration apply, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
