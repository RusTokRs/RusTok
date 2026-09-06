# M6 replay page-duration / lease-heartbeat policy

Status: `source_complete_execution_pending`.

## Purpose

Replay already bounded production source calls plus mutation/checkpoint storage futures, and the multi-page runner already heartbeated between pages. That was not sufficient for a large page: one page may contain many mutations, so a valid run could spend longer than its lease inside `run_next_page` before the next page-count heartbeat.

This slice defines and enforces the page-duration / lease policy without adding a coarse whole-page timeout that would hide the existing dependency-specific timeout identities.

## Canonical dependency windows

The production replay page now has bounded outer observation windows for each dependency phase that can otherwise remain pending:

- production `IndexSource::scan`: `30s`, with `index_source_scan_timeout`;
- checkpoint read transaction: `30s`, with `index_replay_checkpoint_read_timeout`;
- one mutation persistence future: `30s`, with `index_replay_mutation_timeout`;
- checkpoint commit transaction: `30s`, with `index_replay_checkpoint_commit_timeout`.

Checkpoint identity validation remains outside the read/commit timeout wrappers. A timeout is still only an observation bound: dropping the future does not prove that an underlying database operation was rolled back or cancelled.

The newly bounded checkpoint read closes the one replay data-plane phase that could otherwise remain pending while an in-page heartbeat kept extending the job lease indefinitely.

## Minimum run lease

`IndexReplayRunRequest` now rejects lease durations shorter than `60s`.

The minimum deliberately reserves two current canonical dependency windows: one window for a single bounded page dependency plus one full window of lease/fencing margin. The existing durable job request continues to enforce whole-second leases and the 24-hour upper bound.

The GraphQL command already uses the exact `60s` minimum, so the public server transport does not change its configured replay budget.

This is a fail-closed contract. If the canonical dependency window changes, the lease policy and retained guard must be reviewed together rather than silently allowing the run lease to become smaller than the page dependency budget.

## In-page heartbeat

Both ordinary and graceful replay runners wrap each one-page future in the same `await_page_with_lease_heartbeats` helper.

For an active page:

1. compute the heartbeat interval as one third of the configured lease duration;
2. continue polling the real page future;
3. when the interval elapses while the page is still pending, race the page future with `PostgresIndexReplayJobStore::heartbeat`;
4. after a successful heartbeat, schedule the next heartbeat from the completion time;
5. include successful in-page heartbeats in `IndexReplayRunOutcome::heartbeat_count`.

For the server-owned `60s` lease this produces a `20s` in-page heartbeat interval.

The pre-existing `heartbeat_every_pages` boundary heartbeat remains intact. It still provides an explicit page-count cadence for fast pages; the in-page timer protects a slow or mutation-heavy page from losing its lease merely because the next page boundary has not been reached yet.

## Why there is no whole-page timeout

A single outer page timeout would race the more precise source/mutation/checkpoint timeout contracts. It could turn a known `index_replay_mutation_timeout` or `index_replay_checkpoint_commit_timeout` into a generic page-duration failure and would make durable-state recovery harder to diagnose.

This policy therefore keeps dependency-specific timeout identity authoritative and uses lease heartbeats only for ownership maintenance. There is no new page terminal state and no generic `index_replay_page_timeout` code.

## Lease-maintenance failure

If an in-page heartbeat reports `LeaseLost`, the runner returns the existing fenced `IndexReplayRunError::LeaseLost`. If heartbeat storage itself fails, the runner returns the existing job-storage error path.

In either case the page future is no longer trusted. Mutations that may already be durable remain protected by stable delivery IDs, inbox deduplication and monotonic source versions; checkpoint writes remain protected by active-lease fencing.

No synthetic checkpoint, rollback, success or failure transition is invented after lost lease ownership.

## Cancellation and graceful shutdown

This slice does not merge persisted cancellation, graceful shutdown and lease maintenance into one state machine.

- the in-page heartbeat does not set, clear or reinterpret `cancel_requested`;
- the ordinary runner retains its existing persisted-cancellation checks before and after each page and before terminal failure publication;
- `StopHandle` interruption is still observed only through the worker's cooperative safe points;
- the graceful runner uses the same in-page lease helper around that interruptible page future;
- a graceful interruption still yields the owned job to `pending` and preserves the last committed checkpoint;
- no automatic retry/requeue policy is added.

## Production source boundary

Production sources registered through `register_index_source` retain the canonical 30-second source-call wrapper. Direct `IndexSourceCatalog::register` remains a low-level/test boundary and is not promoted here as a production way to bypass source-call bounds.

## Retained source evidence

Source assertions now retain:

- minimum replay lease rejection below `60s` and acceptance at `60s`;
- one-third in-page heartbeat interval (`60s -> 20s`);
- ordinary and graceful page futures both routed through the common lease-heartbeat helper;
- successful in-page heartbeat accounting;
- checkpoint-read timeout identity and retryability;
- unchanged mutation/checkpoint-commit timeout identities;
- persisted cancellation precedence after page failure;
- no generic whole-page timeout or new retry/requeue behavior.

`verify-index-replay-page-lease-heartbeat.mjs` cross-checks the runner, source/storage timeout constants, GraphQL's fixed lease, graceful path, current plan and this document.

The retained Rust tests and Node verifiers were not executed by the implementation agent.

## Still open

- maintainer execution/admission of this page-duration/lease-heartbeat source evidence;
- maintainer execution/admission of the existing dependency-timeout packets;
- broader multi-host/restart execution evidence;
- partition replay only after a real partition-capable source contract exists;
- explicit targeted/full/shadow rebuild modes under a separate contract.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, SQLite/PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
