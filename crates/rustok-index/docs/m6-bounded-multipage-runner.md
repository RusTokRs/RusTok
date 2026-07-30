# M6 bounded multi-page replay runner

Status: `source_complete_owner_execution_pending`

This slice composes the existing one-page replay worker, fenced rebuild jobs, mutation
store, and lease-bound checkpoint store into one bounded PostgreSQL runner. It includes
durable cancellation requests and between-page terminal cancellation. It does not add a
scheduler, background task, automatic retry/backoff, in-page interruption, dry-run, or
production source adapter.

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
2. observes and terminalizes a previously requested cancellation before each page;
3. executes no more than the requested page budget through
   `IndexReplayWorker::run_next_page`;
4. extends the lease between pages at the requested completed-page cadence;
5. observes cancellation again after heartbeat and after every completed page;
6. accumulates applied, duplicate, stale, mutation, page, and heartbeat counts;
7. calls fenced terminal success only after the one-page worker persisted a JSON `null`
   cursor;
8. when the page budget ends with continuation, atomically returns the same job to
   `pending`, clears lease ownership, and makes it immediately claimable for resume.

A page itself is not interrupted by a heartbeat or cancellation task. Operators must
choose a lease longer than the maximum admitted source-page and mutation/checkpoint
commit duration. A cancellation requested during a page is observed after that page's
idempotent mutations and checkpoint are durable.

## Cancellation contract

`PostgresIndexReplayRunner::request_cancel` accepts one non-nil tenant and job UUID. It
locks the exact `rebuild` job before changing state:

- a `pending` job becomes terminal `cancelled` immediately;
- a `running` job retains its current owner and records `cancel_requested = TRUE`;
- `succeeded`, `failed`, and `cancelled` return their typed terminal state;
- an unknown tenant/job pair returns `NotFound`.

The active owner terminalizes a running request only when the exact job UUID, worker ID,
attempt count, running state, and unexpired lease still match. Cancellation clears lease
ownership, preserves the durable checkpoint, records `completed_at`, and stores no error
payload.

Success, page failure, and pending-yield SQL all require
`cancel_requested = FALSE`. This makes cancellation linear with every competing terminal
or resume transition: a cancellation committed first cannot be overwritten by success,
failure, or `pending`; a terminal transition committed first makes a later request return
that terminal state.

## Resume and attempt fencing

A yielded job keeps the same job UUID and durable checkpoint, but the next acquisition
increments `attempt_count`. The old attempt cannot heartbeat, yield, fail, complete,
cancel, or advance the checkpoint after the new attempt is claimed.

A running cancellation request survives lease expiry and reclaim. The next owner sees the
persisted flag before reading the source and terminalizes the job with the incremented
attempt fence.

## Failure boundary

Source, mutation, and checkpoint failures are recorded as one bounded
`index.replay_page_failed` terminal job error with an `index_replay_run_failure_v1`
detail object containing only the dependency code and retryable classification. No raw
database, transport, or source-domain message is persisted.

Cancellation takes precedence when it races with page failure. Checkpoint failures
classified as `checkpoint_lease_lost`, heartbeat loss, terminal completion loss, yield
loss, and cancellation ownership loss return explicit `IndexReplayRunError::LeaseLost`
instead of attempting a stale state write.

## Still open

- interruption or timeout of one currently executing source page;
- automatic bounded retry/backoff and dead-letter scheduling;
- a host scheduler, command/transport authorization, and graceful process shutdown;
- direct server-composition publication of the runner;
- dry-run and targeted/full/shadow rebuild modes;
- locale and partition checkpoint dimensions;
- Product and later source adapters;
- retained PostgreSQL cancellation, crash, lease-expiry, restart, timing, and
  multi-instance evidence.

## Owner validation

```bash
node scripts/verify/verify-index-replay-multipage-runner.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_replay_runner --lib -- --nocapture
```

These commands are maintainer-run for this slice.
