# M6 cooperative replay-page interruption

Status: `worker_and_runner_source_complete_host_binding_pending`

## Purpose

`IndexReplayWorker` exposes an interruptible one-page execution path without changing the
existing ordinary replay API. `run_next_page` delegates to the same implementation with a no-op
probe, while `PostgresIndexReplayRunner::run_interruptible` now supplies a host-owned probe to
`run_next_page_interruptible` under the exact active replay lease.

The probe returns a machine-bounded result:

- `Ok(false)` continues execution;
- `Ok(true)` returns `IndexReplayError::Interrupted`;
- `Err(IndexReplayFailure)` returns `IndexReplayError::InterruptionCheckFailed` with only the
  existing bounded retryable/permanent dependency code.

The one-page worker does not impose a deadline on the probe future itself. The runner adapter uses
a synchronous boolean host probe, so it does not introduce another pending dependency future at
these safe points.

## Safe interruption boundaries

The worker checks the probe at three durable boundaries:

1. after checkpoint readiness and immediately before source scan;
2. before every mutation is passed to `IndexReplayMutationSink`;
3. after the next checkpoint is constructed and immediately before checkpoint commit.

A completed null-cursor checkpoint remains authoritative and returns `AlreadyComplete` without
consulting the probe or source.

An interruption never commits the page checkpoint. If interruption occurs after one or more
mutations were already persisted, the next run scans the same page again. Existing inbox
deduplication and monotonic source-version guards remain the safety owner for that redelivery.

## Runner integration

The original `source_replay_runner.rs` ordinary run/cancel path remains unchanged. A nested runner
extension adds `PostgresIndexReplayRunner::run_interruptible` and reuses the same source resolution,
job acquisition, heartbeat, mutation store, checkpoint store, completion and pending-yield helpers.

When a page returns `Interrupted`, the runner checks persisted cancellation first. If no operator
cancel won the race, the runner uses the fenced pending-yield transition rather than failure or
terminal cancellation. The job keeps its UUID, clears lease ownership, preserves the last committed
checkpoint, records no failure details, and can be claimed as a new attempt.

The retained runner-level SQLite packet proves both pre-scan yield/restart and the durable-mutation / missing-checkpoint redelivery window where attempt 2 observes `Duplicate` before completion.

## Interaction with merged M6 slices

Production Product, ProductVariant, SalesChannel, and future canonical sources are registered
through the 30-second source-call timeout wrapper. The pre-scan safe point therefore combines with
a bounded production source future; interruption does not replace the source timeout.

The bounded replay dry-run remains a separate no-write validation capability. It does not call
this worker and does not claim cancellation, checkpoint, or mutation interruption semantics.

Mutation-sink and checkpoint-store futures are not preempted. Interruption is observed only before
starting the next mutation or checkpoint commit, not while one of those operations is already
pending.

## Ownership boundaries

The one-page worker remains database and runtime neutral. It does not:

- read `index_jobs` or interpret `cancel_requested`;
- terminalize a replay job as cancelled;
- create a PostgreSQL cancellation probe;
- interrupt a source, mutation, checkpoint, or probe future already in progress;
- add a timer, polling loop, scheduler, task, or graceful-shutdown owner;
- change source, mutation, checkpoint, cursor, event identity, migration, or table shape.

The PostgreSQL runner extension owns lease-aware state transitions around interruption, but the
server lifecycle still does not supply its `StopHandle` to this path. `SharedIndexReplayRuntime`,
`IndexReplayOperatorRuntime`, and GraphQL therefore remain on ordinary replay execution until the
next host-composition slice.

The canonical roadmap item remains partially open for actual server stop binding, pending-future
timeout handling, explicit rebuild modes, locale/partition scope, and retained execution evidence.

## Source evidence

Focused source scenarios retain:

- worker interruption before source scan with no source, mutation, or checkpoint write;
- worker interruption immediately before checkpoint commit after one durable mutation, followed by
  duplicate-safe replay and checkpoint completion;
- bounded retryable interruption-probe failure before source access;
- runner interruption before scan yielding the durable job back to `pending`;
- runner interruption after durable mutation / before checkpoint commit, followed by attempt-2
  duplicate redelivery and successful completion.

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL cancellation/lease races, restart
scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
