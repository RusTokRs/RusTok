# M6 reconciliation dead-letter inspection

Status: `source_complete_transport_pending`.

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

Database failures map to one stable `Storage` error with no embedded database detail. The inspector performs no insert, update, delete, retry, requeue, reset, scheduling, polling, sleep, or task creation.

## Authorized server composition

The server-owned `IndexReconciliationOperatorRuntime` composes this inspector beside the canonical reconciliation runner and audited recovery store.

Inspection accepts only:

- one validated `IndexReconciliationOperatorContext`;
- one job UUID.

The delegated tenant is derived exclusively from the context. There is no caller-supplied tenant parameter.

Before adapter validation or database access, the server boundary reads the exact request-scoped tenant/actor permission snapshot and requires effective `modules:manage`. Missing request authority and insufficient permission fail before delegation. The server returns only the bounded inspection object or typed bounded errors; it does not expose the database adapter or raw diagnostic JSON.

The same guarded runtime also exposes manual audited requeue. That write operation is owned by `PostgresIndexReconciliationRecoveryStore`, not by the inspector, and requires the same request-bound context plus an explicit bounded reason. Tenant and actor are derived only from the authorized context.

GraphQL, HTTP, CLI, MCP, and admin transports remain open. This slice publishes internal guarded capabilities only.

## Compatibility

Inspection remains read-only and changes no reconciliation query, failed-scope admission, retry policy, lease fence, cursor, source, mutation, schema, cancellation, or success behavior.

The authorized recovery composition is additive: it delegates to the separately merged scope-locked same-job reset and immutable actor/reason audit contract without modifying inspector SQL or returned data.

## Explicitly open

- inspection and recovery transport mapping;
- automatic retry, backoff, exhaustion, scheduling, and graceful shutdown;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair admission;
- locale or partition checkpoint dimensions;
- retained PostgreSQL inspection, authorization, concurrency, and recovery evidence;
- complete drift repair.

The canonical bounded retry/global scheduling and drift-diagnosis/targeted-repair roadmap items remain open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
