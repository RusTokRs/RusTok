# M6 cooperative replay-page interruption

Status: `worker_runner_host_binding_source_complete_execution_pending`

## Purpose

`IndexReplayWorker` exposes an interruptible one-page execution path without changing the
existing ordinary replay API. `run_next_page` delegates to the same implementation with a no-op
probe, while `PostgresIndexReplayRunner::run_interruptible` supplies a host-owned probe to
`run_next_page_interruptible` under the exact active replay lease.

`SharedIndexReplayRuntime` and `IndexReplayOperatorRuntime` now carry the same lifecycle-neutral
boolean probe through runtime and authorization boundaries. The server GraphQL replay command binds
that probe to the shared `StopHandle::is_stopping` signal without exposing shutdown state in caller
input.

The probe returns a machine-bounded result:

- `Ok(false)` continues execution;
- `Ok(true)` returns `IndexReplayError::Interrupted`;
- `Err(IndexReplayFailure)` returns `IndexReplayError::InterruptionCheckFailed` with only the
  existing bounded retryable/permanent dependency code.

The one-page worker does not impose a deadline on the probe future itself. The runner/shared-runtime
adapter uses a synchronous boolean host probe, so it does not introduce another pending dependency
future at these safe points.

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

The retained runner-level SQLite packet proves both pre-scan yield/restart and the durable-mutation /
missing-checkpoint redelivery window where attempt 2 observes `Duplicate` before completion.

## Server binding

GraphQL schema initialization resolves one `StopHandle` from shared `ServerRuntimeContext`. API-only
hosts atomically create the same typed handle when none exists and retain one private watch receiver,
so the existing `StopHandle::stop()` sender can publish its terminal value even when no background
worker subscribed.

`runIndexReplay` obtains that server-owned handle only from schema data and calls the guarded
interruptible operator with `|| stop_handle.is_stopping()`. The transport does not call `.stop()` and
accepts no stop handle, shutdown flag, or probe from GraphQL input.

The Index crate and `IndexReplayOperatorRuntime` do not import `StopHandle`; only the boolean probe
crosses those boundaries. User-requested `cancelIndexReplay` remains the separate persisted
cancellation path.

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

The PostgreSQL runner extension owns lease-aware state transitions around interruption. The shared
Index runtime and guarded server operator only forward a boolean probe. Server GraphQL composition
owns the actual lifecycle signal binding.

The canonical roadmap item remains open for execution/admission evidence, pending-future timeout
handling, explicit rebuild modes, locale/partition scope, and broader multi-host timing evidence.

## Source evidence

Focused source scenarios retain:

- worker interruption before source scan with no source, mutation, or checkpoint write;
- worker interruption immediately before checkpoint commit after one durable mutation, followed by
  duplicate-safe replay and checkpoint completion;
- bounded retryable interruption-probe failure before source access;
- runner interruption before scan yielding the durable job back to `pending`;
- runner interruption after durable mutation / before checkpoint commit, followed by attempt-2
  duplicate redelivery and successful completion;
- server source guards proving the authorized GraphQL run path samples only
  `StopHandle::is_stopping` and keeps shutdown controls out of caller input.

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL cancellation/lease races, restart
scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
