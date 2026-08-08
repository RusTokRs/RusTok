# M6 replay pending-future timeout boundary

Status: `source_complete_execution_pending`.

## Purpose

Replay already had bounded owner-source calls and cooperative safe-point interruption, but those controls did not bound a storage future after mutation persistence or checkpoint commit had already started.

This slice adds an outer timeout to the two PostgreSQL replay storage phases that can otherwise remain pending inside one page:

- `PostgresMutationStore` when used through `IndexReplayMutationSink`;
- `PostgresIndexReplayCheckpointStore::commit_replay_checkpoint`.

The one-page worker and multi-page runner state machines are unchanged.

## Timeout contract

`source_replay_timeout.rs` owns one canonical 30-second outer bound for each storage future and two stable retryable dependency codes:

- `index_replay_mutation_timeout`;
- `index_replay_checkpoint_commit_timeout`.

The bound applies only after replay-specific contract preparation has succeeded. A normal dependency failure still passes through with its existing permanent/retryable classification.

Source scan/load calls remain independently bounded by the existing canonical Index source timeout wrapper. This slice does not merge source and storage timeouts into one page deadline.

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

No new runner terminal state is introduced. Existing `replay_failure_details` already maps mutation/checkpoint dependency failures to their machine code and `IndexReplayFailureKind` retryability.

For either timeout, the multi-page runner still checks persisted cancellation after the page error and before writing terminal failure. Therefore:

1. a user cancellation that won the race remains `Cancelled`;
2. otherwise an active fenced attempt records the existing `index.replay_page_failed` terminal job failure with timeout dependency code and `retryable: true` in details;
3. lease loss still fails closed through the existing lease path;
4. `StopHandle` interruption remains a separate safe-point-only `Yielded -> pending` path and is not used as a storage deadline.

This slice does not automatically requeue a failed replay job. Existing replay retry/recovery policy remains the owner of later retry admission.

## Lease boundary

The 30-second value is an upper bound for one storage dependency future, not a guarantee that a whole page fits inside a job lease. Existing heartbeat and lease fencing remain authoritative. A future page-budget/lease policy may tighten these per-phase values, but must not weaken the timeout error semantics retained here.

## Retained source evidence

`source_replay_timeout.rs` retains unit source for:

- a never-completing mutation future becoming retryable `index_replay_mutation_timeout`;
- a never-completing checkpoint commit future becoming retryable `index_replay_checkpoint_commit_timeout`;
- an immediate dependency failure passing through unchanged instead of being rewritten as timeout.

`verify-index-replay-pending-future-timeout.mjs` additionally locks the production adapter wiring, timeout codes, runner retryability mapping, cancellation precedence and the deliberate absence of `StopHandle` / `request_cancel` from the timeout helper.

The retained tests and verifier were not executed by the implementation agent.

## Still open

- maintainer execution/admission of the retained timeout source evidence;
- broader page-duration versus lease/heartbeat budgeting under multi-mutation pages;
- checkpoint-read pending-future policy if later evidence shows it needs a separate bound;
- locale/partition replay checkpoint dimensions;
- explicit targeted/full/shadow rebuild modes;
- broader multi-host/restart execution evidence.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
