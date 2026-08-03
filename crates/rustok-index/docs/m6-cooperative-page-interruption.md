# M6 cooperative replay-page interruption

Status: `source_complete_runner_probe_pending`

## Purpose

`IndexReplayWorker` exposes an interruptible one-page execution path without changing the
existing replay API. The ordinary `run_next_page` method delegates to the same implementation
with a no-op probe, while a later lease-aware runner can call
`run_next_page_interruptible`.

The caller-owned probe returns a machine-bounded result:

- `Ok(false)` continues execution;
- `Ok(true)` returns `IndexReplayError::Interrupted`;
- `Err(IndexReplayFailure)` returns `IndexReplayError::InterruptionCheckFailed` with only the
  existing bounded retryable/permanent dependency code.

The worker does not impose a deadline on the probe future itself. A production caller must keep
that probe bounded and must not attach raw database, transport, request, tenant, or stack text to
its result.

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

## Interaction with merged M6 slices

Production Product, ProductVariant, SalesChannel, and future canonical sources are already
registered through the 30-second source-call timeout wrapper. The pre-scan safe point therefore
combines with a bounded production source future; this interruption contract does not replace the
source timeout.

The bounded replay dry-run remains a separate no-write validation capability. It does not call
this worker and does not claim cancellation, checkpoint, or mutation interruption semantics.

Mutation-sink and checkpoint-store futures are not preempted by this slice. The worker can observe
interruption only before starting the next mutation or checkpoint commit, not while one of those
operations is already pending.

## Ownership boundaries

This slice is database and runtime neutral. It does not:

- read `index_jobs` or interpret `cancel_requested`;
- bind the probe to an active replay lease;
- terminalize a replay job as cancelled;
- create a PostgreSQL cancellation probe;
- interrupt a source, mutation, checkpoint, or probe future already in progress;
- add a timer, polling loop, scheduler, task, or graceful-shutdown owner;
- change source, mutation, checkpoint, cursor, event identity, migration, or table shape.

The PostgreSQL runner must later derive the probe from the exact active
`(tenant_id, job_id, worker_id, attempt_count)` lease, classify probe storage failures through a
bounded code, and preserve its existing fenced cancellation transition.

The canonical combined roadmap item for in-page interruption/timeouts, dry-run, and
targeted/full/shadow rebuild modes remains open because runner binding, pending-future handling,
rebuild modes, and retained PostgreSQL evidence are still absent.

## Source evidence

Focused source scenarios retain:

- interruption before source scan with no source, mutation, or checkpoint write;
- interruption immediately before checkpoint commit after one durable mutation, followed by
  duplicate-safe replay and checkpoint completion;
- bounded retryable interruption-probe failure before source access.

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL cancellation/lease races, restart
scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
