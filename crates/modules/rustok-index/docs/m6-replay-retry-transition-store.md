# M6 replay retry transition store

Status: `source_complete_execution_pending`

## Purpose

`PostgresIndexReplayRunner` records failure through the original job lease, not through a
second job or a separate completion acknowledgment. The runner maps bounded dependency
codes to typed retry/terminal outcomes while the scheduler observes durable state.

`PostgresIndexReplayRetryStore::record_failure` accepts only an active
`IndexReplayJobLease` and a bounded machine-readable dependency failure. The caller cannot pass
raw source, database, transport, request, tenant, mutation, or stack details.

## Policy

The default policy is fixed and bounded:

- maximum failures: `5`;
- base delay: `5 seconds`;
- maximum delay: `300 seconds`;
- retry delays after failures 1-4: `5`, `10`, `20`, and `40` seconds;
- permanent failures terminalize immediately;
- a retryable failure at failure 5 terminalizes as exhausted.

Custom policies are bounded to 1-100 failures and whole-second delays from 1 to 86400 seconds.
The base delay cannot exceed the maximum delay. Failure ordinal zero and invalid failure codes fail
closed.

## Durable transitions

For a retryable failure below the failure limit, the store updates the same `index_jobs` row:

```text
running -> pending
available_at = current time + deterministic backoff
lease_owner = null
lease_expires_at = null
completed_at = null
```

For a permanent or exhausted failure, it updates that same row:

```text
running -> failed
lease_owner = null
lease_expires_at = null
completed_at = current time
```

Both transitions require the exact tenant, job, worker, attempt count, unexpired running lease,
and `cancel_requested = false`. Both increment `failure_count`; a normal yield does not.
A stale, expired, or cancelled worker receives `LeaseLost` and
cannot publish retry or terminal state.

The existing replay acquisition contract already treats a pending row as unavailable until
`available_at <= CURRENT_TIMESTAMP`. This store therefore inserts no job, sleeps for no delay,
polls no table, and starts no task. The next successful claim keeps the same job UUID and
increments the durable attempt count in the existing job store.

## Bounded diagnostics

`last_error_details` uses the stable `index_replay_retry_v1` contract and contains only:

- bounded dependency code;
- retryable or permanent classification;
- lease generation, failure count, and configured maximum failures;
- selected disposition;
- optional retry delay and next failure ordinal.

The transition API stores no tenant, job, worker, schema, source payload, database error, SQL,
transport context, request value, mutation payload, backtrace, or arbitrary owner detail.

## Runner, scheduler, and dead letter

Retryable dependency failures become `RetryScheduled`, with a durable delay; permanent and
exhausted failures remain `failed`. Cancellation and stale-lease outcomes do not consume the
failure budget. The page driver and dependency deadlines stay independent. Shadow replay
remains read-only and does not participate in retries.

The host scheduler applies scope-level failed-job admission for current selected schemas and
sources. Authorized requeue resets the failure budget, advances a retry epoch, and preserves
the original job and checkpoint. See the [operations guide](./operations.md).

The canonical implementation-plan item remains open for retained execution evidence and the
rest of the global scheduling/cutover boundary; source does not establish fleet readiness.

## Validation ownership

Cargo checks/tests, JavaScript source verifiers, and live PostgreSQL scenarios are maintainer-run.
