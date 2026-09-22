# M6 replay retry transition store

Status: `source_complete_runner_wiring_pending`

## Purpose

This slice adds an Index-owned durable transition boundary for bounded replay retries. It does not
change `PostgresIndexReplayRunner`, start a scheduler, or make failed scopes automatically
recoverable.

`PostgresIndexReplayRetryStore::record_failure` accepts only an active
`IndexReplayJobLease` and a bounded machine-readable dependency failure. The caller cannot pass
raw source, database, transport, request, tenant, mutation, or stack details.

## Policy

The default policy is fixed and bounded:

- maximum attempts: `5`;
- base delay: `5 seconds`;
- maximum delay: `300 seconds`;
- retry delays after attempts 1-4: `5`, `10`, `20`, and `40` seconds;
- permanent failures terminalize immediately;
- a retryable failure at attempt 5 terminalizes as exhausted.

Custom policies are bounded to 1-100 attempts and whole-second delays from 1 to 86400 seconds.
The base delay cannot exceed the maximum delay. Attempt zero and invalid failure codes fail
closed.

## Durable transitions

For a retryable failure below the attempt limit, the store updates the same `index_jobs` row:

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
and `cancel_requested = false`. A stale, expired, or cancelled worker receives `LeaseLost` and
cannot publish retry or terminal state.

The existing replay acquisition contract already treats a pending row as unavailable until
`available_at <= CURRENT_TIMESTAMP`. This store therefore inserts no job, sleeps for no delay,
polls no table, and starts no task. The next successful claim keeps the same job UUID and
increments the durable attempt count in the existing job store.

## Bounded diagnostics

`last_error_details` uses the stable `index_replay_retry_v1` contract and contains only:

- bounded dependency code;
- retryable or permanent classification;
- current attempt and configured maximum;
- selected disposition;
- optional retry delay and next-attempt number.

The transition API stores no tenant, job, worker, schema, source payload, database error, SQL,
transport context, request value, mutation payload, backtrace, or arbitrary owner detail.

## Interaction with merged M6 slices

The source-call timeout and cooperative replay-page interruption slices classify or bound work at
other boundaries. They do not call this store. The bounded dry-run runtime remains read-only and
also does not participate in retries.

The current `PostgresIndexReplayRunner` still terminalizes page failures through its existing
`finish_failure` path. A later wiring slice must map the bounded replay/source/storage failure
classification into `IndexReplayRetryFailure`, call this store under the active lease, preserve
cancellation precedence, and return a truthful scheduled-versus-terminal outcome.

## Explicitly open

- runner failure-classification and transition-store wiring;
- automatic host scheduling when `available_at` becomes eligible;
- scope-level failed-job admission and bounded dead-letter inspection;
- authorized requeue with actor/reason audit and retry-epoch reset;
- per-source or per-run retry configuration and jitter;
- fleet-wide scheduling and graceful task shutdown;
- retained PostgreSQL retry, exhaustion, cancellation-race, lease-expiry, restart, and recovery
  evidence.

A terminal `failed` row is durable evidence for one job, but this slice alone is not a complete
dead-letter queue. Until failed-scope admission lands, a later explicit invocation can still
create another job for the same schema scope.

The canonical implementation-plan item `Add bounded retry/backoff, dead-letter state, and global
scheduling ownership` therefore remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, live PostgreSQL scenarios, workflows, and CI
are maintainer-run and were not executed by the implementation agent.
