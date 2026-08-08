# M6 replay graceful interruption and restart

Status: `runner_source_complete_host_binding_execution_pending`.

## Purpose

The one-page replay worker already exposes cooperative interruption safe points around durable boundaries. This
slice carries that contract through `PostgresIndexReplayRunner` without changing ordinary replay or persisted
operator cancellation semantics.

`PostgresIndexReplayRunner::run_interruptible` accepts one host-owned synchronous probe. The probe is adapted to
`IndexReplayWorker::run_next_page_interruptible`, which checks it:

1. after checkpoint readiness and before source scan;
2. before each mutation application;
3. after page mutations are durable and before checkpoint commit.

The existing `PostgresIndexReplayRunner::run` remains unchanged and does not manufacture an interruption probe.

## Host interruption is not user cancellation

`IndexReplayError::Interrupted` is handled separately from page failure. The runner:

1. first checks whether a persisted operator cancellation won the race;
2. if cancellation is present, returns the existing terminal `Cancelled` outcome;
3. otherwise calls the existing fenced `yield_for_resume` transition;
4. returns `Yielded` with the same durable job UUID and attempt number;
5. leaves the job `pending`, clears lease ownership, records no failure payload, and preserves the last committed
   checkpoint.

Host interruption never sets `cancel_requested` and never publishes `failed`. User-requested cancellation keeps
its existing contract and continues to be persisted through `request_cancel`.

## Restart and redelivery

An interruption before source scan performs no source work and leaves no checkpoint change. A new worker can
claim the same pending job, increment the attempt fence, and continue normally.

The important crash-equivalent window is interruption after a mutation has committed but before the page
checkpoint commits. The durable mutation remains in `index_inbox` / materialization while the replay checkpoint
still points at the previous page. A later attempt therefore scans the same page again. Stable delivery identity
causes the already-durable mutation to return `Duplicate`; only then does the resumed page commit its checkpoint
and complete the job.

This intentionally relies on the existing inbox deduplication and monotonic source-version guarantees rather
than introducing rollback or an unsafe synthetic checkpoint advance.

## Retained source evidence

`source_replay_graceful_shutdown_tests.rs` retains deterministic SQLite source for two cases:

- interruption at the first safe point proves zero source scans, a pending lease-free job, no checkpoint, and
  successful attempt-2 restart;
- interruption at the third safe point of a one-mutation page proves one durable entity/inbox receipt, no
  checkpoint, pending yield, then attempt-2 duplicate redelivery and successful checkpoint/job completion.

The packet uses production Index migrations, source registry, mutation store, replay jobs, checkpoint store and
runner state transitions. It has not been executed by the implementation agent.

## Still open

This slice does **not** yet connect a server `StopHandle` to `run_interruptible`. The next server-composition
boundary must provide a host-owned stop probe without exposing it to GraphQL callers and without bypassing
`IndexReplayOperatorRuntime` authority.

Also still separate:

- runtime execution/admission of this retained evidence;
- GraphQL replay command execution/cancellation evidence;
- locale/partition replay checkpoint scope;
- explicit rebuild modes;
- broader multi-host/timing evidence.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
