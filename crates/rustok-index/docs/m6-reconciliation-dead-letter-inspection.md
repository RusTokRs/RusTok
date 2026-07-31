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

This Index adapter intentionally accepts no actor or permission object. It is not a public endpoint.

Server, GraphQL, HTTP, CLI, and admin callers must bind the exact request tenant and actor, require a request-scoped effective `modules:manage` permission, and reject cross-tenant requests before calling the inspector. That server-owned authorization wrapper remains open and must reuse the guarded reconciliation operator boundary rather than expose the database adapter directly.

## Compatibility

The slice adds no migration and changes no reconciliation job state, lease, retry, cursor, source, mutation, schema, or admission behavior. It is read-only and can be deployed independently of manual requeue or retry-epoch reset.

## Remaining work

- server-owned authorized inspection composition and transport mapping;
- actor/reason audit records;
- manual requeue or retry-epoch reset under the reconciliation scope lock;
- automatic retry/backoff/exhaustion and host scheduling;
- digest comparison, orphan cleanup, targeted/full/shadow repair;
- locale/partition checkpoint dimensions and complete drift-repair admission;
- retained PostgreSQL execution evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_reconciliation_dead_letter_inspector -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-reconciliation-dead-letter-inspection.mjs
```

The implementation agent did not run formatting, Cargo commands, JavaScript verifiers, PostgreSQL fixtures, or CI, per maintainer instruction.
