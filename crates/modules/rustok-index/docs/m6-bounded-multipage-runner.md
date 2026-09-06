# M6 bounded multi-page replay runner

Status: `source_complete_owner_execution_pending`

This slice composes the existing one-page replay worker, fenced rebuild jobs, mutation
store, and lease-bound checkpoint store into one bounded PostgreSQL runner. It includes
durable cancellation requests, between-page terminal cancellation, a separate host-probed
in-page interruption path, server-owned graceful-shutdown binding through the guarded
replay command, and time-based in-page lease maintenance. It does not add a scheduler,
background task, automatic retry/backoff, dry-run, or production source adapter.

## Request bounds

`IndexReplayRunRequest` fixes one invocation to:

- one non-nil tenant and exact `SchemaRef`;
- one bounded worker identity and whole-second lease duration of at least 60 seconds;
- a source page limit already constrained to 1 through 1000;
- 1 through 1024 pages per invocation;
- a heartbeat cadence from 1 through the invocation page budget.

The source name is never caller supplied. `PostgresIndexReplayRunner` resolves the exact
owner from `SharedIndexSourceRegistry` before acquiring the durable job. The job request
therefore uses the same source identity as page execution and checkpoint persistence.

The 60-second minimum is the page-duration lease floor. Production source calls and replay
checkpoint-read/mutation/checkpoint-commit futures each have a canonical 30-second outer
observation bound. Long pages keep their lease alive every one third of the configured lease
while preserving those dependency-specific timeout identities; see
`m6-replay-page-lease-heartbeat.md`.

## Ordinary execution order

For an acquired `IndexReplayJobLease`, ordinary `PostgresIndexReplayRunner::run`:

1. constructs `PostgresIndexReplayCheckpointStore` from that exact lease;
2. observes and terminalizes a previously requested cancellation before each page;
3. executes no more than the requested page budget through
   `IndexReplayWorker::run_next_page`;
4. while a page is pending, extends the active lease every one third of the lease duration;
5. also extends the lease between pages at the requested completed-page cadence;
6. observes cancellation again after boundary heartbeat and after every completed page;
7. accumulates applied, duplicate, stale, mutation, page, and both boundary/in-page heartbeat counts;
8. calls fenced terminal success only after the one-page worker persisted a JSON `null`
   cursor;
9. when the page budget ends with continuation, atomically returns the same job to
   `pending`, clears lease ownership, and makes it immediately claimable for resume.

A persisted cancellation requested during ordinary execution is still observed after the
current page's idempotent mutations and checkpoint are durable. In-page lease heartbeats do
not inspect or reinterpret `cancel_requested`; the existing user-cancel contract is unchanged.

## Host-probed in-page interruption

`PostgresIndexReplayRunner::run_interruptible` is a separate execution entry point over the
same job/checkpoint/mutation stores. It adapts a host-owned probe to
`IndexReplayWorker::run_next_page_interruptible`, whose safe points are before source scan,
before every mutation, and before checkpoint commit.

The interruptible page future is awaited through the same time-based lease-maintenance helper
as ordinary replay. Lease ownership can therefore stay valid during a long page without
turning the heartbeat into a shutdown or cancellation probe.

If the worker returns `IndexReplayError::Interrupted`, the runner first preserves any
persisted cancellation race. Otherwise it uses the ordinary fenced pending-yield transition:

- the job keeps the same UUID;
- state returns to `pending`;
- lease ownership is cleared;
- no failure payload is recorded;
- the last committed checkpoint is preserved;
- the next claim increments `attempt_count`.

Host interruption does not set `cancel_requested` and is not a terminal cancellation.

If interruption occurs after one or more mutations are already durable but before checkpoint
commit, the next attempt replays the same page. Stable delivery IDs make those durable
mutations `Duplicate`; the resumed page can then advance its checkpoint safely. No synthetic
checkpoint advance or mutation rollback is introduced.

The retained SQLite packet is documented in `m6-replay-graceful-shutdown.md`. It is
source-only until maintainer execution/admission.

## Server shutdown binding

The Index runner and `SharedIndexReplayRuntime` accept only a lifecycle-neutral boolean probe.
`IndexReplayOperatorRuntime::run_interruptible` preserves exact request authorization before
passing that probe to Index.

GraphQL schema initialization owns the actual server lifecycle binding. It resolves the one
shared `StopHandle`, retains a private watch receiver for API-only hosts, and publishes a clone
in schema data. The authorized `runIndexReplay` command samples only
`StopHandle::is_stopping`; GraphQL input contains no shutdown state or stop control and the
transport never calls `StopHandle::stop()`.

`cancelIndexReplay` remains the separate persisted cancellation state machine.

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

If an in-page heartbeat itself loses the lease, page execution fails closed through the
existing `IndexReplayRunError::LeaseLost` path. The runner does not manufacture a checkpoint,
rollback, retry or terminal state after losing ownership.

The retained multi-host packet now proves the concurrent form of that fence. Host A remains
blocked inside a real page future while the evidence fixture expires its lease. A distinct
host B runner reclaims the same job as attempt 2 and completes it. When host A is released,
its late stable delivery is duplicate-safe and its stale checkpoint path returns
`IndexReplayRunError::LeaseLost`; attempt-2 durable state remains authoritative. See
`m6-replay-multihost-reclaim-evidence.md`.

## Failure boundary

Source, mutation, and checkpoint failures are recorded as one bounded
`index.replay_page_failed` terminal job error with an `index_replay_run_failure_v1`
detail object containing only the dependency code and retryable classification. No raw
database, transport, or source-domain message is persisted.

Checkpoint read, mutation persistence and checkpoint commit each retain their own retryable
30-second timeout code. There is deliberately no generic whole-page timeout that could mask
one of those dependency identities.

Cancellation takes precedence when it races with page failure. Checkpoint failures
classified as `checkpoint_lease_lost`, heartbeat loss, terminal completion loss, yield
loss, and cancellation ownership loss return explicit `IndexReplayRunError::LeaseLost`
instead of attempting a stale state write.

Host interruption is not a page failure and does not enter this failure path.

## Still open

- execute/admit retained interruption/restart, page lease-heartbeat, multi-host reclaim and end-to-end server-shutdown evidence;
- automatic bounded retry/backoff and dead-letter scheduling remains a separate owner policy;
- operator-visible scheduler health and metrics;
- explicit targeted/full/shadow rebuild modes;
- partition replay scope only after a real partition-capable source contract exists;
- retained PostgreSQL/process-level cancellation, crash, lease-expiry, restart and timing evidence beyond the deterministic SQLite packets where deployment admission requires it.

The guarded schema/locale GraphQL run/cancel command transport, locale checkpoint identity,
server `StopHandle` observation, in-page lease maintenance and source-only multi-host reclaim
fencing are complete; execution evidence remains maintainer-owned.

## Owner validation

```bash
node scripts/verify/verify-index-replay-multipage-runner.mjs
node scripts/verify/verify-index-replay-graceful-shutdown.mjs
node scripts/verify/verify-index-replay-page-lease-heartbeat.mjs
node scripts/verify/verify-index-replay-multihost-reclaim-evidence.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_replay_runner --lib -- --nocapture
```

These commands are maintainer-run for this slice.
