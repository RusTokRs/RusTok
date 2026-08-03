# M6 reconciliation runner retry wiring

Status: `runner_complete_host_scheduler_pending`.

## Purpose

`PostgresIndexReconciliationRunner` now applies the merged reconciliation retry policy to source, mutation, and page-contract failures. The runner no longer owns a separate unconditional `running -> failed` SQL path.

Every page failure is classified once, converted to `IndexReconciliationRetryFailure`, and committed through `PostgresIndexReconciliationRetryStore` under the exact current tenant/job/worker/attempt lease fence.

This slice changes failure disposition only. Acquisition, schema admission, source cursor progression, mutation application, success, yield, heartbeat, cancellation, expired-lease reclaim, manual recovery, and dead-letter admission remain separate existing boundaries.

## Classification

The runner maps failures as follows:

| Runner failure | Retry classification | Dependency code |
| --- | --- | --- |
| source `SourceFailure` | source-owned retryable/permanent kind | source-owned bounded code |
| mutation failure | mutation-owned retryable/permanent kind | mutation-owned bounded code |
| other source contract failure | permanent | `source_contract_invalid` |
| nil/duplicate event id or other page contract failure | permanent | `reconciliation_contract_invalid` |

Invalid retry codes fail closed through the typed `RetryTransition` error. The runner does not substitute raw error text or store Debug/Display output.

## Typed outcomes

A successful durable failure transition returns `IndexReconciliationRunOutcome` rather than returning the original dependency error:

- `RetryScheduled` — same job remains pending and the outcome contains `retry_after` and `next_attempt`;
- `FailedPermanent` — same job is terminal failed and retry metadata is absent;
- `FailedExhausted` — retryable dependency exhausted attempt 5 and the same job is terminal failed.

The outcome retains:

- exact job UUID;
- current durable attempt count;
- pages, passes, heartbeat, mutation, applied, duplicate, and stale counters accumulated before the failure.

The outcome deliberately omits dependency code, raw source/mutation failure, request, cursor, tenant, worker, SQL, database cause, and stack text.

## Default attempt sequence

The runner composes `PostgresIndexReconciliationRetryStore::new`, so the default sequence is fixed:

```text
attempt 1 failure -> pending for 5 seconds, next attempt 2
attempt 2 failure -> pending for 10 seconds, next attempt 3
attempt 3 failure -> pending for 20 seconds, next attempt 4
attempt 4 failure -> pending for 40 seconds, next attempt 5
attempt 5 retryable failure -> failed exhausted
any permanent failure -> failed permanent
```

An invocation before `available_at` receives the existing `Busy` outcome and does not call the source. When due, the existing claim path reuses the same job UUID and increments `attempt_count` exactly once.

## Cancellation and stale leases

The retry-store update requires one current unexpired running lease and `cancel_requested = false`.

If the transition loses because cancellation committed first, the runner invokes the existing fenced cancellation completion and returns `Cancelled`. If no cancellation can be completed, it returns the existing `LeaseLost` error for the exact job and attempt.

Database or policy failures from the retry boundary are returned as detail-free `RetryTransition`; they do not expose the retry store's database cause.

## Dead-letter compatibility

Permanent and exhausted jobs retain:

- the same job UUID;
- their final attempt count;
- `last_error_code = index.reconciliation_page_failed`;
- the strict `index_reconciliation_run_failure_v1` diagnostic.

Later ordinary runs therefore continue to return `DeadLettered` without selecting or exposing `last_error_details`. Inspection and audited manual requeue remain compatible and unchanged.

## Static and source tests

The source-level SQLite tests cover:

- retry attempt outcomes at 5, 10, 20, and 40 seconds;
- `Busy` before `available_at` without another source call;
- same-job UUID and monotonic attempt fencing;
- attempt-5 exhausted terminal state;
- immediate permanent terminal state;
- dead-letter admission after both terminal dispositions;
- unchanged success and bounded-yield behavior.

The retained PostgreSQL dead-letter harness now expects a typed `FailedPermanent` first outcome and keeps its existing database-state and privacy assertions.

These tests are committed source evidence only and were not executed by the implementation agent.

## Explicitly open

- host-owned due reconciliation discovery;
- bounded worker invocation and concurrency admission;
- fleet-wide scheduler ownership, takeover, and graceful shutdown;
- per-source policy, jitter, and dynamic configuration;
- retained PostgreSQL multi-attempt, exhaustion, cancellation-race, lease-expiry, restart, and multi-instance evidence;
- transport or operator APIs for scheduling;
- drift diagnosis and targeted/full/shadow repair.

The canonical retry/backoff/dead-letter/global-scheduling roadmap item remains open until host scheduling ownership is implemented.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
