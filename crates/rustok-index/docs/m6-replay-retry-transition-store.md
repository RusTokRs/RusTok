# M6 replay retry transition store

Status: `source_complete_runner_wiring_pending`

This slice adds an Index-owned durable transition boundary for bounded replay retries. It does not
start a scheduler and it does not change the current runner yet.

## Policy

The default policy is deliberately fixed and bounded:

- maximum attempts: `5`;
- base delay: `5 seconds`;
- maximum delay: `300 seconds`;
- retry delays after attempts 1-4: `5`, `10`, `20`, and `40` seconds;
- permanent failures terminalize immediately;
- a retryable failure at the attempt limit terminalizes as exhausted.

Custom policies are bounded to 1-100 attempts and whole-second delays from 1 to 86400 seconds.
The base delay cannot exceed the maximum delay.

## Durable transitions

`PostgresIndexReplayRetryStore::record_failure` accepts an active
`IndexReplayJobLease` and a bounded machine-readable dependency code.

For a retryable failure below the attempt limit it updates the same `index_jobs` row:

```text
running -> pending
available_at = current time + deterministic backoff
lease_owner = null
lease_expires_at = null
completed_at = null
```

For a permanent or exhausted failure it updates the same row:

```text
running -> failed
lease_owner = null
lease_expires_at = null
completed_at = current time
```

Both updates require the exact tenant, job, worker, attempt count, active lease, and
`cancel_requested = false`. A stale worker receives `LeaseLost` and cannot publish retry or
terminal state.

The existing replay job acquisition contract already treats a pending row as unavailable until
`available_at <= CURRENT_TIMESTAMP`, so this store does not sleep or poll.

## Safe diagnostics

The store accepts only a lowercase bounded failure code. It never accepts raw source, database,
transport, request, tenant, or stack-trace details. `last_error_details` contains only the stable
`index_replay_retry_v1` shape:

- dependency code and retryable/permanent classification;
- current and maximum attempts;
- selected disposition;
- optional retry delay and next attempt number.

## Explicitly open

- wiring retry classification from `PostgresIndexReplayRunner` into this store;
- host scheduling of pending jobs when `available_at` becomes eligible;
- scope-level dead-letter blocking and an authorized operator requeue command;
- per-source/per-run retry policy configuration;
- jitter or fleet-wide retry coordination;
- retained PostgreSQL retry, exhaustion, cancellation-race, lease-expiry, and restart evidence.

A terminal `failed` row is durable evidence for the current job, but this slice does not claim a
complete dead-letter queue: the current acquisition path can still create a new job after an
explicit later operator invocation. The combined implementation-plan item for retry/backoff,
dead-letter state, and global scheduling ownership therefore remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, workflow execution, and live PostgreSQL
validation are maintainer-run. The implementation agent did not execute them.
