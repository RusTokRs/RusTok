# M6 reconciliation dead-letter inspection

Status: `source_complete_transport_pending`.

## Purpose

`PostgresIndexReconciliationDeadLetterInspector` provides a bounded, read-only view of one terminal failed reconciliation job after ordinary admission blocks its exact tenant/schema scope.

The query is restricted to exact tenant/job, `kind = 'reconcile'`, and `state = 'failed'`. Cross-tenant, active, pending, successful, cancelled, non-reconciliation, and unknown jobs return no inspection.

## Returned contract

A successful inspection contains only failed job UUID, positive attempt count, optional bounded `last_error_code`, bounded dependency code from `index_reconciliation_run_failure_v1`, and retryability.

Diagnostics use `deny_unknown_fields`; malformed contracts, codes, JSON, zero attempts, and overflow fail closed. Raw diagnostic JSON is never returned.

The query selects only attempt count, machine code, and diagnostic object. It does not expose tenant identity, request/cursor JSON, source/worker/lease/timestamps, payloads, SQL, database causes, transport, or stack text.

The inspector performs no insert, update, delete, retry, requeue, reset, scheduling, polling, sleep, or task creation.

## Authorized server composition

`IndexReconciliationOperatorRuntime` composes the inspector beside the canonical runner and audited recovery store. Inspection accepts one validated context and job UUID; there is no caller-supplied tenant parameter.

Before adapter validation or database access, the server requires the exact request-scoped tenant/actor snapshot and effective `modules:manage`. The server returns only the bounded inspection object or typed bounded errors.

GraphQL, HTTP, CLI, MCP, and admin transports remain open.

## Scheduler compatibility

The module-owned host scheduler changes no inspector SQL or returned data. Retryable jobs remain pending and are not inspectable. Permanent or exhausted jobs become failed through the canonical retry store and retain the strict inspection-compatible diagnostic.

Manual audited requeue remains separately owned by `PostgresIndexReconciliationRecoveryStore`.

## Explicitly open

- inspection and recovery transport mapping;
- retained PostgreSQL inspection, authorization, scheduler contention, and recovery evidence;
- operator-visible scheduler health and metrics;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair admission;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical bounded retry/global scheduling item remains open pending owner-retained production and multi-host evidence. The drift-diagnosis/targeted-repair item remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
