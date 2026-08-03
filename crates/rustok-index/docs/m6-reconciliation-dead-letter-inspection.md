# M6 reconciliation dead-letter inspection

Status: `source_complete_transport_and_recovery_pending`.

## Purpose

`PostgresIndexReconciliationDeadLetterInspector` provides a bounded, read-only view of one terminal failed reconciliation job after ordinary admission has blocked its exact tenant/schema scope.

The adapter requires an exact non-nil tenant UUID and job UUID. Its query is restricted to the exact tenant/job with `kind = 'reconcile'` and `state = 'failed'`. Cross-tenant, active, pending, successful, cancelled, non-reconciliation, and unknown jobs therefore return no inspection.

The adapter is published through `rustok_index::infrastructure::postgres`. It is a storage capability, not a public endpoint or an authorization boundary.

## Returned contract

A successful inspection contains only:

- failed job UUID;
- positive durable attempt count;
- optional bounded `last_error_code`;
- bounded `dependency_code` from `index_reconciliation_run_failure_v1`;
- retryability boolean.

Both machine codes are limited to 128 ASCII bytes and lowercase letters, digits, `.`, `_`, or `-`.

Stored diagnostics use `#[serde(deny_unknown_fields)]` and must match the exact three-field reconciliation failure contract. Unknown fields, malformed JSON, unsupported contract versions, invalid machine codes, zero attempts, and overflowing attempts fail closed through `InvalidStoredJob`.

## Privacy and read-only boundary

Unlike ordinary dead-letter admission, inspection deliberately reads `last_error_details` so it can decode the bounded dependency code and retryability flag. The raw JSON object is never returned.

The query selects only attempt count, `last_error_code`, and `last_error_details`. It does not select or return:

- tenant identity;
- schema request or cursor JSON;
- source name, worker identity, lease ownership, or timestamps;
- entity, relation, inbox, or mutation payloads;
- SQL, database causes, transport context, or stack text.

Database failures map to one stable `Storage` error with no embedded database detail. The production implementation performs no insert, update, delete, retry, requeue, reset, scheduling, polling, sleep, or task creation.

## Authorized server composition

The server-owned `IndexReconciliationOperatorRuntime` now composes this inspector beside the canonical reconciliation runner.

Inspection accepts only:

- one validated `IndexReconciliationOperatorContext`;
- one job UUID.

The delegated tenant is derived exclusively from the context. There is no caller-supplied tenant parameter.

Before adapter validation or database access, the server boundary reads the exact request-scoped tenant/actor permission snapshot and requires effective `modules:manage`. Missing request authority and insufficient permission fail before delegation. The server returns only the bounded inspection object or typed bounded errors; it does not expose the database adapter or raw diagnostic JSON.

GraphQL, HTTP, CLI, MCP, and admin transports remain open. This slice publishes an internal guarded capability only.

## Compatibility

This inspection slice adds no migration and changes no reconciliation state transition, failed-scope admission, retry policy, lease fence, cursor, source, mutation, schema, cancellation, or success behavior.

It composes additively with the replay retry store, replay dead-letter admission, reconciliation dead-letter admission, guarded reconciliation runtime, and engine-level reconciliation recovery store already present on `main`.

## Explicitly open

- inspection transport mapping;
- server-owned authorization and transport for manual requeue or retry-epoch reset;
- automatic retry, backoff, exhaustion, scheduling, and graceful shutdown;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair admission;
- locale or partition checkpoint dimensions;
- retained PostgreSQL inspection, authorization, and recovery evidence;
- complete drift repair.

The canonical M6 drift-diagnosis and targeted-repair roadmap item remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
