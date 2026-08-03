# Index reconciliation operator runtime

Status: `source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes one guarded `IndexReconciliationOperatorRuntime` after the replay composition freezes the immutable source and schema registries.

The boundary wraps:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only inspection;
- `PostgresIndexReconciliationRecoveryStore` for audited same-job recovery.

The same registry-freezing composition now also publishes the separate module-owned reconciliation work registration. The operator runtime does not expose or own that scheduler.

## Request-bound authority

Every operation requires a non-nil tenant/actor `IndexReconciliationOperatorContext`, a current request-scoped RBAC snapshot, and effective `Permission::MODULES_MANAGE`.

Run rejects a tenant mismatch before runner delegation. Cancellation, inspection, and requeue accept no caller-selected tenant. Requeue accepts no caller-selected actor; the audit actor is always `context.actor_id()`.

Inspection and requeue authorization occur before adapter or recovery-request validation and before database access.

## Published surface

The operator exposes only:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

It exposes no database connection, registry, scheduler handle, worker-spawn handle, raw failure details, direct SQL, or transport.

## Composition and scheduling

The server replay composition remains the single source-freezing point:

1. PostgreSQL source factories are materialized;
2. `SharedIndexSourceRegistry` is frozen;
3. replay dry-run/runtime and the due-reconciliation module-work registration are published;
4. this guarded reconciliation operator is built from the same source/schema registries and database.

Composition performs no reconciliation SQL and starts no task. The existing generic server module-work bootstrap later owns the one-second polling loop and shared `StopHandle` shutdown. The Index adapter discovers work; the canonical runner owns claim, takeover, attempt fencing, cancellation, retry, exhaustion, and terminal state.

The operator remains intentionally independent from automatic scheduling: manual authorized calls and host-scheduled calls converge only at the same canonical runner.

## Explicitly open

- GraphQL, HTTP, CLI, MCP, native admin, or other command transport;
- retained PostgreSQL authorization and scheduler execution evidence;
- operator-visible scheduler health and metrics;
- per-source retry policy, jitter, and dynamic configuration;
- source/index digest comparison, orphan diagnosis, and targeted/full/shadow repair;
- locale or partition checkpoint dimensions.

The canonical bounded retry/global scheduling item remains open pending owner-retained production and multi-host evidence. The drift-diagnosis/targeted-repair item also remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
