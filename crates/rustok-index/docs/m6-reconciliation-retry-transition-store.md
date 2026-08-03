# M6 reconciliation retry transition store

Status: `runner_complete_scheduler_wiring_pending`.

## Purpose

`PostgresIndexReconciliationRetryStore` provides the Index-owned durable transition boundary for bounded reconciliation retry, backoff, and terminal exhaustion.

`PostgresIndexReconciliationRunner` now classifies page failures and delegates their state transition to this store. The remaining open ownership boundary is host scheduling: neither component starts a task, polls `index_jobs`, sleeps until `available_at`, or claims fleet-wide scheduler leadership.

## Retry lease

`IndexReconciliationRetryLease` requires:

- one non-nil tenant UUID;
- one non-nil existing reconciliation job UUID;
- one bounded, trimmed, control-character-free worker identity;
- one positive durable attempt count.

The runner constructs this lease only from its currently acquired reconciliation attempt. The transition SQL independently verifies the same tenant, job, worker, and attempt together with an unexpired running lease and `cancel_requested = false`.

## Failure contract

`IndexReconciliationRetryFailure` accepts only a validated machine-readable dependency code and one classification:

- `Retryable`;
- `Permanent`.

The runner maps source and mutation failure kinds directly. Source contract failures use the permanent code `source_contract_invalid`; other page-contract failures use `reconciliation_contract_invalid`.

Codes are limited to 128 ASCII bytes containing lowercase letters, digits, `.`, `_`, or `-`. The retry boundary accepts no raw source, database, SQL, request, transport, payload, tenant, worker, stack, or arbitrary owner detail.

## Bounded policy

The runner uses the fixed default policy:

- maximum attempts: `5`;
- base backoff: `5 seconds`;
- maximum backoff: `300 seconds`;
- delays after attempts 1-4: `5`, `10`, `20`, and `40` seconds;
- permanent failures terminalize immediately;
- a retryable failure at attempt 5 terminalizes as exhausted.

Custom store policies remain bounded to 1-100 attempts and whole-second delays from 1 through 86,400 seconds, but per-source or dynamic runner policy selection remains open. The base delay cannot exceed the maximum delay.

## Durable transitions

For a retryable failure below the attempt limit, `record_failure` updates the same `index_jobs` row:

```text
running -> pending
available_at = current time + deterministic backoff
lease_owner = null
lease_expires_at = null
completed_at = null
```

The job UUID, cursor, durable attempt count, and retry epoch are preserved. The normal reconciliation claim path already requires `available_at <= CURRENT_TIMESTAMP` and increments the attempt only when the pending row is claimed.

For a permanent failure or exhausted retry budget, the same store updates that row:

```text
running -> failed
lease_owner = null
lease_expires_at = null
completed_at = current time
```

Both paths are fenced by exact tenant, job, worker, attempt, active lease, running state, and cancellation state. A stale, expired, cancelled, or otherwise replaced attempt receives `LeaseLost` and cannot publish pending or terminal state.

The store inserts no job row, changes no job UUID, sleeps for no delay, polls no table, and starts no task.

## Runner outcomes

The runner returns typed outcomes after a successful durable transition:

- `RetryScheduled` with bounded `retry_after` and `next_attempt`;
- `FailedPermanent` without retry metadata;
- `FailedExhausted` without retry metadata.

The outcome keeps the exact job UUID, current attempt, and counters already accumulated during the invocation. It does not return the dependency code or raw failure.

Cancellation still wins if it commits before the retry transition. A stale or expired lease maps back to the runner's existing `LeaseLost` error. Other transition failures are wrapped in the detail-free `RetryTransition` error.

## Diagnostic compatibility

Both scheduled and terminal transitions preserve the existing strict `index_reconciliation_run_failure_v1` object:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "<bounded-code>",
  "retryable": true
}
```

`last_error_code` remains `index.reconciliation_page_failed`.

Keeping the exact three-field contract means the merged dead-letter inspector remains compatible when a permanent or exhausted job reaches `failed`. Retry policy, delay, attempt budget, tenant, job, worker, request, source payload, SQL, database causes, and stack text are not added to the diagnostic JSON.

## Interaction with recovery

Automatic retry remains within the existing retry epoch and preserves the attempt count until a later claim increments it.

The merged audited manual requeue contract is different: it operates only on terminal failed rows, increments `retry_epoch`, resets `attempt_count` to zero, installs the initial cursor, and appends an immutable actor/reason audit. Automatic retry does not create recovery audits or perform manual reset semantics.

## Explicitly open

- host-owned due-job discovery and bounded invocation;
- fleet-wide scheduling ownership, takeover, and graceful shutdown;
- per-source policy, jitter, and dynamic configuration;
- retained PostgreSQL retry, exhaustion, cancellation-race, lease-expiry, restart, and recovery evidence;
- GraphQL, HTTP, CLI, MCP, or admin transport;
- source/index digest comparison, orphan diagnosis, and targeted/full/shadow repair.

The canonical implementation-plan item `Add bounded retry/backoff, dead-letter state, and global scheduling ownership` remains open because global scheduling ownership is not part of this slice.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
