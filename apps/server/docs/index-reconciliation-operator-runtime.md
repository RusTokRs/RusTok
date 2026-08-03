# Index reconciliation operator runtime

Status: `source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes one guarded `IndexReconciliationOperatorRuntime` after the replay composition freezes the immutable source and schema registries.

The boundary wraps:

- `PostgresIndexReconciliationRunner` for bounded run and cancellation;
- `PostgresIndexReconciliationDeadLetterInspector` for bounded read-only dead-letter inspection;
- `PostgresIndexDriftFindingInspector` for bounded read-only open-finding diagnosis;
- `PostgresIndexReconciliationRecoveryStore` for audited same-job recovery.

The same registry-freezing composition also publishes the separate module-owned reconciliation work registration. The operator runtime does not expose or own that scheduler.

## Request-bound authority

Every operation requires a non-nil tenant/actor `IndexReconciliationOperatorContext`, a current request-scoped RBAC snapshot, and effective `Permission::MODULES_MANAGE`.

Run rejects a tenant mismatch before runner delegation. Cancellation, dead-letter inspection, drift-finding inspection, and requeue accept no caller-selected tenant. Requeue accepts no caller-selected actor; the audit actor is always `context.actor_id()`.

Both inspection methods and requeue authorize before adapter or recovery-request validation and before database access. An unauthorized caller therefore cannot use nil identifiers, malformed requests, or storage behavior as a tenant-scoped oracle.

## Published surface

The operator exposes only:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `inspect_drift_finding(context, finding_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

Drift inspection returns only the bounded crate value: finding UUID and key, check name, severity, typed scope, and optional expected/actual digests. It does not return tenant identity, raw finding details, detection timestamps, closure state, SQL, or database causes.

The runtime exposes no database connection, registry, scheduler handle, worker-spawn handle, raw failure or finding details, direct SQL, or transport.

## Composition and scheduling

The server replay composition remains the single source-freezing point:

1. PostgreSQL source factories are materialized;
2. `SharedIndexSourceRegistry` is frozen;
3. replay dry-run/runtime and the due-reconciliation module-work registration are published;
4. this guarded reconciliation operator is built from the same source/schema registries and database;
5. the canonical runner, both read-only inspectors, and the audited recovery store are inserted into one private runtime.

Composition performs no reconciliation or drift SQL and starts no task. The existing generic server module-work bootstrap later owns the one-second polling loop and shared `StopHandle` shutdown. The Index adapter discovers work; the canonical runner owns claim, takeover, attempt fencing, cancellation, retry, exhaustion, and terminal state.

The operator remains intentionally independent from automatic scheduling: manual authorized calls and host-scheduled calls converge only at the same canonical runner. Drift inspection is read-only and is not scheduled.

## Explicitly open

- GraphQL, HTTP, CLI, MCP, native admin, or other command transport;
- retained PostgreSQL authorization, inspection, and scheduler execution evidence;
- operator-visible scheduler health and metrics;
- per-source retry policy, jitter, and dynamic configuration;
- source/index digest comparison and consistency-finding production;
- orphan diagnosis;
- finding resolution or ignore transitions;
- targeted/full/shadow repair admission, execution, audit, and evidence;
- locale or partition checkpoint dimensions.

The canonical bounded retry/global scheduling item remains open pending owner-retained production and multi-host evidence. The drift-diagnosis/targeted-repair item also remains open because this runtime only authorizes bounded inspection of findings that already exist.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
