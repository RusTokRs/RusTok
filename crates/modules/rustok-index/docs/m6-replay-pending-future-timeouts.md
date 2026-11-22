# M6 replay pending-future timeout boundary

Status: `source_complete_execution_pending`.

## Purpose

Replay already had bounded owner-source calls and cooperative safe-point interruption, but storage futures inside one page also need finite outer observation bounds so lease maintenance cannot keep an indefinitely pending storage operation alive.

The production PostgreSQL replay adapter now bounds all three storage phases that can otherwise remain pending inside one page:

- checkpoint read transaction;
- `PostgresMutationStore` when used through `IndexReplayMutationSink`;
- `PostgresIndexReplayCheckpointStore::commit_replay_checkpoint`.

The one-page worker error surface remains unchanged.

## Timeout contract

`source_replay_timeout.rs` owns one canonical 30-second outer bound for each replay storage dependency future and three stable retryable dependency codes:

- `index_replay_checkpoint_read_timeout`;
- `index_replay_mutation_timeout`;
- `index_replay_checkpoint_commit_timeout`.

The bound applies only after replay-specific identity/contract preparation that can fail synchronously has succeeded. A normal dependency failure still passes through with its existing permanent/retryable classification.

Production source scan/load calls remain independently bounded by the existing canonical Index source timeout wrapper at 30 seconds. The whole-page deadline is a separate 90-second observation budget; it does not replace source and storage timeout codes; see `m6-replay-page-lease-heartbeat.md`.

## Checkpoint read timeout semantics

Checkpoint identity validation remains outside the timeout so invalid lease/scope input fails immediately as the existing permanent contract error.

The database transaction, active lease check, checkpoint read/decode and transaction commit/rollback are then observed through one bounded checkpoint-read future. Timeout returns retryable `index_replay_checkpoint_read_timeout`, preserved by the worker as `IndexReplayError::CheckpointReadFailed`.

A timed-out read does not synthesize a checkpoint and does not claim that the database cancelled every underlying operation immediately when the future was dropped. The next attempt still reads durable state normally under the existing lease fence.

Bounding the read is also required by the page lease-heartbeat policy: otherwise a pending checkpoint read could keep receiving lease extensions indefinitely without ever reaching source scan, mutation persistence, or checkpoint commit.

## Mutation timeout semantics

The replay adapter creates the stable `MutationDelivery` first and then wraps `PostgresMutationStore::apply` in the outer timeout.

If the future completes, the existing `Applied` / `Duplicate` / `StaleIgnored` mapping is unchanged. If the outer timeout expires, the adapter returns retryable `index_replay_mutation_timeout`, which the worker preserves as `IndexReplayError::MutationFailed` at the current mutation position.

A timeout does **not** prove that the database operation was rolled back or cancelled. The future is dropped, but an underlying driver/database operation may have crossed a durable boundary before the caller stopped observing it. Therefore replay correctness continues to rely on the pre-existing stable event/delivery ID, inbox deduplication and monotonic source-version checks.

The worker does not synthesize a checkpoint for a timed-out mutation. If storage did commit before the timeout was observed, a later replay of the same page must safely resolve that delivery through the existing duplicate/stale rules.

## Checkpoint commit timeout semantics

Checkpoint identity validation remains outside the timeout so invalid lease/scope input fails immediately as the existing permanent contract error.

The database transaction, active lease check, checkpoint upsert and transaction commit are then wrapped as one bounded checkpoint-commit future. Timeout returns retryable `index_replay_checkpoint_commit_timeout`, preserved by the worker as `IndexReplayError::CheckpointCommitFailed`.

The timeout path does not execute a synthetic rollback, rewind or checkpoint write after the outer future has expired. It also does not claim that the underlying database commit could not have completed. The next attempt must read the durable checkpoint normally and continue from whatever state storage actually committed under the existing lease fence.

## Runner failure and cancellation precedence

Replay carries the dependency code and retry classification through the original job lease.
After a page error it checks persisted cancellation before recording a failure:

1. a user cancellation that won the race remains `Cancelled`;
2. an active fenced attempt whose dependency failed can return `RetryScheduled`, or
   terminalize on the last allowed failure; the dependency remains retryable in diagnostics;
3. lease loss still fails closed through the existing lease path;
4. `StopHandle` interruption remains a separate safe-point-only `Yielded -> pending` path
   and is not used as a storage deadline.

## Lease boundary

`IndexReplayRunRequest` requires at least a 60-second lease. The shared page driver renews an
active lease every third of its duration (20 seconds at the minimum lease) and continues
polling the page while a heartbeat waits. The 90-second whole-page deadline emits
`replay_page_deadline` only when it wins; a shorter completed dependency timeout keeps its own
code. See [page lease heartbeats](./m6-replay-page-lease-heartbeat.md).

## Retained source evidence

`source_replay_timeout.rs` retains unit source for:

- a never-completing checkpoint-read future becoming retryable `index_replay_checkpoint_read_timeout`;
- a never-completing mutation future becoming retryable `index_replay_mutation_timeout`;
- a never-completing checkpoint-commit future becoming retryable `index_replay_checkpoint_commit_timeout`;
- an immediate dependency failure passing through unchanged instead of being rewritten as timeout.

`verify-index-replay-pending-future-timeout.mjs` locks production adapter wiring, all three timeout codes, runner retryability mapping, cancellation precedence and the deliberate absence of `StopHandle` / `request_cancel` from the timeout helper.

`verify-index-replay-page-lease-heartbeat.mjs` separately locks the minimum lease, in-page heartbeat policy and the whole-page deadline.

The retained tests and verifiers were not executed by the implementation agent.

## Still open

- maintainer execution/admission of the retained dependency-timeout source evidence;
- maintainer execution/admission of the page lease-heartbeat source evidence;
- broader multi-host/restart execution evidence;
- partition replay only after a real partition-capable source contract exists;

Behavioral verification requires maintainer execution.
