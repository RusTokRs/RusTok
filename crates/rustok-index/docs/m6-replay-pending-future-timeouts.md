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

Production source scan/load calls remain independently bounded by the existing canonical Index source timeout wrapper at 30 seconds. The page-duration/lease policy does not merge source and storage timeouts into one generic page deadline; see `m6-replay-page-lease-heartbeat.md`.

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

No new runner terminal state is introduced. Existing `replay_failure_details` maps checkpoint-read, mutation and checkpoint-commit dependency failures to their machine code and `IndexReplayFailureKind` retryability.

For any of these timeout paths, the multi-page runner still checks persisted cancellation after the page error and before writing terminal failure. Therefore:

1. a user cancellation that won the race remains `Cancelled`;
2. otherwise an active fenced attempt records the existing `index.replay_page_failed` terminal job failure with timeout dependency code and `retryable: true` in details;
3. lease loss still fails closed through the existing lease path;
4. `StopHandle` interruption remains a separate safe-point-only `Yielded -> pending` path and is not used as a storage deadline.

This slice does not automatically requeue a failed replay job. Existing replay retry/recovery policy remains the owner of later retry admission.

## Lease boundary

Per-dependency 30-second bounds are not summed into a whole-page timeout. Large pages can legitimately span many bounded mutation futures, so the runner now maintains lease ownership while the real page future is pending.

`IndexReplayRunRequest` requires at least a 60-second lease, and the ordinary/graceful runners heartbeat an active page every one third of the configured lease duration. With the server-owned 60-second lease, the in-page cadence is 20 seconds. The existing page-count heartbeat remains intact for fast pages.

This keeps exact source/checkpoint/mutation timeout identities authoritative. There is deliberately no generic `index_replay_page_timeout` code. Full policy and retained evidence are documented in `m6-replay-page-lease-heartbeat.md`.

## Retained source evidence

`source_replay_timeout.rs` retains unit source for:

- a never-completing checkpoint-read future becoming retryable `index_replay_checkpoint_read_timeout`;
- a never-completing mutation future becoming retryable `index_replay_mutation_timeout`;
- a never-completing checkpoint-commit future becoming retryable `index_replay_checkpoint_commit_timeout`;
- an immediate dependency failure passing through unchanged instead of being rewritten as timeout.

`verify-index-replay-pending-future-timeout.mjs` locks production adapter wiring, all three timeout codes, runner retryability mapping, cancellation precedence and the deliberate absence of `StopHandle` / `request_cancel` from the timeout helper.

`verify-index-replay-page-lease-heartbeat.mjs` separately locks the minimum lease, in-page heartbeat policy and the deliberate absence of a generic whole-page timeout.

The retained tests and verifiers were not executed by the implementation agent.

## Still open

- maintainer execution/admission of the retained dependency-timeout source evidence;
- maintainer execution/admission of the page lease-heartbeat source evidence;
- broader multi-host/restart execution evidence;
- partition replay only after a real partition-capable source contract exists;
- explicit targeted/full/shadow rebuild modes;

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
