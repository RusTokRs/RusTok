# Reconciliation dead-letter inspection

Status: `source_complete_owner_execution_pending`.

## Purpose

`PostgresIndexReconciliationDeadLetterInspector` provides a bounded, read-only view of one failed reconciliation job after dead-letter admission has blocked the tenant/schema scope.

The adapter requires an exact non-nil tenant UUID and job UUID. Its query is restricted to `kind = 'reconcile'` and `state = 'failed'`, so cross-tenant, non-reconciliation, active, successful, cancelled, or unknown jobs return no inspection.

## Returned contract

The public inspection contains only:

- job UUID;
- positive durable attempt count;
- optional bounded `last_error_code`;
- bounded `dependency_code` from `index_reconciliation_run_failure_v1`;
- retryable flag.

Both machine codes are limited to 128 ASCII bytes and lowercase letters, digits, `.`, `_`, or `-`. Stored diagnostics must match the exact three-field failure contract; unknown fields, unsupported contract versions, invalid codes, zero/overflowing attempts, and malformed JSON fail closed.

The query does not select or return tenant identifiers, schema request JSON, source cursors, worker IDs, lease fields, timestamps, mutation payloads, SQL, database causes, transport details, stack text, or arbitrary diagnostic values. Storage failures map to one stable error without embedding the database error.

## Authorization boundary

The Index adapter intentionally accepts no actor or permission object and remains transport-neutral.

The server now composes `IndexReconciliationDeadLetterOperatorRuntime` only when the guarded reconciliation operator exists. Its `inspect_dead_letter(context, job_id)` method:

- derives tenant scope only from the non-nil `IndexReconciliationOperatorContext`;
- reads the current request-bound effective RBAC snapshot for that exact tenant and actor;
- requires `modules:manage` before invoking the inspector;
- accepts no independent caller-supplied tenant;
- returns the same bounded read-only inspection contract.

GraphQL, HTTP, CLI, MCP, and admin transport mapping remain open and must consume the guarded server capability rather than the PostgreSQL adapter directly.

## Compatibility

The slice adds no migration and changes no reconciliation job state, lease, retry, cursor, source, mutation, schema, or admission behavior. Inspection is read-only and can be deployed independently of manual requeue or retry-epoch reset.

## Remaining work

- transport mapping over the guarded server capability;
- actor/reason audit records;
- manual requeue or retry-epoch reset under the reconciliation scope lock;
- automatic retry/backoff/exhaustion and host scheduling;
- digest comparison, orphan cleanup, targeted/full/shadow repair;
- locale/partition checkpoint dimensions and complete drift-repair admission;
- retained PostgreSQL/server execution evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_reconciliation_dead_letter_inspector -- --nocapture
cargo test -p rustok-server index_reconciliation -- --nocapture
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-reconciliation-dead-letter-inspection.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
```

The implementation agent did not run formatting, Cargo commands, JavaScript verifiers, PostgreSQL fixtures, or CI, per maintainer instruction.
