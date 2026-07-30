# M6 bounded multi-page replay runner

Status: `source_complete_owner_execution_pending`

This slice composes the existing one-page replay worker, fenced rebuild jobs, mutation
store, and lease-bound checkpoint store into one bounded PostgreSQL runner. It does not
add a scheduler, background task, automatic retry/backoff, cancellation command, dry-run,
or production source adapter.

## Request bounds

`IndexReplayRunRequest` fixes one invocation to:

- one non-nil tenant and exact `SchemaRef`;
- one bounded worker identity and whole-second lease duration;
- a source page limit already constrained to 1 through 1000;
- 1 through 1024 pages per invocation;
- a heartbeat cadence from 1 through the invocation page budget.

The source name is never caller supplied. `PostgresIndexReplayRunner` resolves the exact
owner from `SharedIndexSourceRegistry` before acquiring the durable job. The job request
therefore uses the same source identity as page execution and checkpoint persistence.

## Execution order

For an acquired `IndexReplayJobLease`, the runner:

1. constructs `PostgresIndexReplayCheckpointStore` from that exact lease;
2. executes no more than the requested page budget through
   `IndexReplayWorker::run_next_page`;
3. extends the lease between pages at the requested completed-page cadence;
4. accumulates applied, duplicate, stale, mutation, page, and heartbeat counts;
5. calls fenced terminal success only after the one-page worker persisted a JSON `null`
   cursor;
6. when the page budget ends with continuation, atomically returns the same job to
   `pending`, clears lease ownership, and makes it immediately claimable for resume.

A page itself is not interrupted by a heartbeat task. Operators must choose a lease
longer than the maximum admitted source-page and mutation-commit duration. Heartbeats
protect bounded work between pages; retained timing evidence remains an owner step.

## Resume and attempt fencing

A yielded job keeps the same job UUID and durable checkpoint, but the next acquisition
increments `attempt_count`. The old attempt cannot heartbeat, yield, fail, complete, or
advance the checkpoint after the new attempt is claimed.

Yield uses all existing ownership predicates: tenant, job UUID, `kind = 'rebuild'`,
`state = 'running'`, worker ID, attempt count, and an unexpired lease. If any predicate
fails, the runner returns explicit lease loss and does not publish a false pending or
terminal state.

## Failure boundary

Source, mutation, and checkpoint failures are recorded as one bounded
`index.replay_page_failed` terminal job error with an `index_replay_run_failure_v1`
detail object containing only the dependency code and retryable classification. No raw
database, transport, or source-domain message is persisted.

Checkpoint failures classified as `checkpoint_lease_lost`, heartbeat loss, terminal
completion loss, and yield loss return explicit `IndexReplayRunError::LeaseLost` instead
of attempting a stale failure write. A later owner reclaims the expired job through the
existing attempt fence.

## Still open

- cancellation request observation and terminal cancellation;
- automatic bounded retry/backoff and dead-letter scheduling;
- a host scheduler, command surface, and graceful process shutdown ownership;
- direct server-composition publication of the runner;
- dry-run and targeted/full/shadow rebuild modes;
- locale and partition checkpoint dimensions;
- Product and later source adapters;
- retained PostgreSQL crash, lease-expiry, restart, timing, and multi-instance evidence.

## Owner validation

```bash
node scripts/verify/verify-index-replay-multipage-runner.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_replay_runner --lib -- --nocapture
```

These commands are maintainer-run for this slice.
