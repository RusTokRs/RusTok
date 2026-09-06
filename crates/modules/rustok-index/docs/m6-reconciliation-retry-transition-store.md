# M6 reconciliation retry transition store

Status: `host_scheduler_source_complete_owner_execution_pending`.

## Purpose

`PostgresIndexReconciliationRetryStore` is the Index-owned durable transition boundary for bounded reconciliation retry, backoff, and terminal exhaustion.

`PostgresIndexReconciliationRunner` classifies page failures and delegates their state transition to this store. The Index module now also publishes a due-work adapter through the generic host `ModuleWorkScheduler`; the store itself remains task-free, polling-free, and sleep-free.

## Retry lease

`IndexReconciliationRetryLease` requires one non-nil tenant UUID, one non-nil existing reconciliation job UUID, a bounded worker identity, and a positive durable attempt count.

Transition SQL independently verifies the same tenant, job, worker, and attempt together with an unexpired running lease and `cancel_requested = false`.

## Failure contract and policy

`IndexReconciliationRetryFailure` accepts only a validated machine-readable dependency code and one classification: `Retryable` or `Permanent`.

The runner maps source and mutation failure kinds directly. Other source contract failures use `source_contract_invalid`; other page-contract failures use `reconciliation_contract_invalid`.

The fixed runner policy is:

- maximum attempts: `5`;
- base backoff: `5 seconds`;
- maximum backoff: `300 seconds`;
- delays after attempts 1-4: `5`, `10`, `20`, and `40` seconds;
- permanent failures terminalize immediately;
- attempt-5 retryable failure terminalizes as exhausted.

## Durable transitions

Retryable failure below the attempt limit updates the same row from `running -> pending`, installs deterministic future `available_at`, clears lease ownership, keeps `completed_at = NULL`, and preserves job UUID, cursor, attempt count, and retry epoch.

Permanent or exhausted failure updates the same row from `running -> failed`, clears the lease, and installs `completed_at = CURRENT_TIMESTAMP`.

Both transitions retain `last_error_code = index.reconciliation_page_failed` and the strict three-field `index_reconciliation_run_failure_v1` diagnostic. No retry policy, delay, identity, cursor, request, SQL, database cause, transport, or stack text enters that object.

## Host scheduling interaction

The module-owned reconciliation work adapter discovers only the authoritative due pending row or expired running row for one exact schema scope. It does not mutate the row.

The generic host scheduler invokes the canonical runner. The runner performs the actual claim or takeover, increments attempts exactly once, and later returns `RetryScheduled`, `FailedPermanent`, or `FailedExhausted` after this store commits the durable failure disposition.

Fleet duplicates remain safe because discovery does not grant ownership; the existing runner lease transition remains the only durable claim.

Automatic retry stays within the current retry epoch. Audited manual requeue remains a distinct terminal-failure operation that increments the epoch, resets attempts/cursor, and records actor/reason.

## Explicitly open

- retained PostgreSQL retry, exhaustion, cancellation-race, lease-expiry, restart, multi-host scheduling, and shutdown evidence;
- per-source policy, jitter, and dynamic configuration;
- operator-visible scheduler health;
- transport controls;
- drift diagnosis and targeted/full/shadow repair.

The canonical implementation-plan item `Add bounded retry/backoff, dead-letter state, and global scheduling ownership` remains open pending owner-retained production and multi-host evidence.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
