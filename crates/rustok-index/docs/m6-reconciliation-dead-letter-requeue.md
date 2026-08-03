# M6 reconciliation dead-letter requeue

Status: `source_complete_authorized_server_composition_pending`.

## Purpose

`PostgresIndexReconciliationRecoveryStore` provides one explicit engine-level recovery operation for an exact terminal failed reconciliation job.

The operation preserves the existing job UUID. It does not create a replacement job. Instead, it moves the same failed row back to `pending`, resets its bounded execution state, increments a durable retry epoch, and appends an immutable actor/reason audit record in the same database transaction.

## Request contract

`IndexReconciliationRequeueRequest` requires:

- one non-nil tenant UUID;
- one non-nil failed job UUID;
- one non-nil actor UUID;
- one explicit reason bounded to 512 UTF-8 bytes.

The reason must be non-empty, trimmed, and free of control characters. The crate adapter does not infer an actor or reason.

## Scope locking

The recovery store first resolves the exact `kind = 'reconcile'` job and validates that it has schema scope.

Before changing state it acquires the same PostgreSQL transaction-scoped advisory lock used by normal reconciliation admission:

```text
reconcile␟tenant␟module␟entity␟schema-version
```

The row is then selected again under the lock. A missing job returns `NotFound`; a non-failed job returns `NotFailed`. A concurrent state or epoch change fails closed.

## Atomic reset

For an exact failed row, one transaction:

1. increments `retry_epoch`;
2. changes `state` from `failed` to `pending`;
3. resets `attempt_count` to zero;
4. installs a fresh `index_reconciliation_cursor_v1` cursor;
5. clears lease, heartbeat, cancellation, error, and completion fields;
6. makes the job immediately available;
7. appends one audit record containing the exact tenant, job, actor, action, reason, prior attempt count, and new retry epoch.

The next ordinary runner admission claims the same pending job and begins attempt one of the new retry epoch.

The audit insert and job reset commit or roll back together.

## Immutable audit ledger

`index_reconciliation_recovery_audits` is append-only.

The migration installs database-level `BEFORE UPDATE` and `BEFORE DELETE` rejection triggers for PostgreSQL and SQLite. Each tenant/job/retry-epoch tuple is unique, preventing duplicate audit admission for the same recovery epoch.

The audit ledger does not contain raw failure diagnostics, request or cursor JSON, worker or lease fields, SQL, database causes, or transport context.

## Ownership boundary

This slice publishes only the engine-level PostgreSQL recovery store.

The existing server operator does not expose requeue. A later server-owned wrapper must:

1. bind the exact request tenant and actor;
2. require effective request-scoped `modules:manage`;
3. pass the actor only from the authorized context;
4. require an explicit bounded reason;
5. delegate without accepting a separate caller-selected tenant.

GraphQL, HTTP, CLI, MCP, and admin transports remain open.

## Explicitly open

- authorized server composition and transport mapping;
- automatic retry, backoff, exhaustion, scheduling, and graceful shutdown;
- retained PostgreSQL concurrency, authorization, and recovery execution evidence;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair admission;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical bounded retry/global scheduling and drift-diagnosis/targeted-repair roadmap items remain open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, migration apply, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
