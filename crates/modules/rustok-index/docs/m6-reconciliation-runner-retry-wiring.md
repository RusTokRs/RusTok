# M6 reconciliation runner retry wiring

Status: `host_scheduler_source_complete_owner_execution_pending`.

## Purpose

`PostgresIndexReconciliationRunner` applies the merged retry policy to source, mutation, and page-contract failures. Every page failure is classified once and committed through `PostgresIndexReconciliationRetryStore` under the exact current tenant/job/worker/attempt lease fence.

Index now publishes a module-owned due-work adapter through the generic host scheduler. The adapter invokes this same runner; it does not add a second state machine or bypass request, cursor, lease, cancellation, retry, or dead-letter contracts.

## Classification and outcomes

The runner maps source and mutation retryable/permanent kinds to their bounded owner codes. Other source failures become permanent `source_contract_invalid`; nil/duplicate event IDs and other page-contract failures become permanent `reconciliation_contract_invalid`.

A successful durable transition returns:

- `RetryScheduled` with `retry_after` and `next_attempt`;
- `FailedPermanent` without retry metadata;
- `FailedExhausted` without retry metadata.

The outcome keeps job UUID, current attempt, and counters accumulated during the invocation. It omits dependency code, raw failure, tenant, request, cursor, worker, SQL, database cause, transport, and stack text.

Default sequence:

```text
attempt 1 failure -> pending for 5 seconds, next attempt 2
attempt 2 failure -> pending for 10 seconds, next attempt 3
attempt 3 failure -> pending for 20 seconds, next attempt 4
attempt 4 failure -> pending for 40 seconds, next attempt 5
attempt 5 retryable failure -> failed exhausted
any permanent failure -> failed permanent
```

## Scheduler ownership

The generic `ModuleWorkScheduler` owns polling cadence and graceful StopHandle shutdown. The Index adapter discovers one authoritative due scope, validates its strict stored request, and calls this runner with bounded page/lease policy.

An invocation before `available_at` is not discovered. A discovery race or duplicate host still reaches the canonical runner claim path; only one pending or expired-running attempt can acquire the exact scope and attempt lease. Other invocations return `Busy` or another truthful current state.

The runner remains task-free and sleep-free. It never polls for its own retry deadline.

## Cancellation and stale leases

The retry-store update requires one current unexpired running lease and `cancel_requested = false`. If cancellation commits first, the runner completes the existing fenced cancellation transition and returns `Cancelled`. Otherwise a lost transition becomes the existing `LeaseLost` error for the exact job and attempt.

Permanent and exhausted jobs retain the same job UUID, final attempt count, generic failure code, and strict inspection-compatible diagnostic. Ordinary admission continues to return `DeadLettered` without selecting raw details. Inspection and audited manual requeue remain unchanged.

## Explicitly open

- retained PostgreSQL multi-attempt, exhaustion, cancellation-race, lease-expiry, restart, multi-host scheduler, and graceful-shutdown evidence;
- operator-visible scheduler metrics and health;
- per-source policy, jitter, and dynamic configuration;
- transport controls;
- drift diagnosis and targeted/full/shadow repair.

The canonical retry/backoff/dead-letter/global-scheduling roadmap item remains open until owner-retained production and multi-host evidence is admitted.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
