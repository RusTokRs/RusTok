# M6 cooperative replay-page interruption

Status: `source_complete_runner_probe_pending`

## Purpose

`IndexReplayWorker` now exposes an interruptible one-page execution path without
changing the existing replay API. The ordinary `run_next_page` method delegates to
the same implementation with a no-op probe, while hosts that own a cancellation or
shutdown signal can call `run_next_page_interruptible`.

The probe returns a bounded result:

- `Ok(false)` continues execution;
- `Ok(true)` returns `IndexReplayError::Interrupted`;
- `Err(IndexReplayFailure)` returns
  `IndexReplayError::InterruptionCheckFailed` without accepting a raw database,
  transport, request, tenant, or stack payload.

## Safe interruption boundaries

The worker checks the probe exactly at these durable boundaries:

1. after checkpoint readiness and immediately before the source scan;
2. before every mutation is passed to `IndexReplayMutationSink`;
3. after the page checkpoint is constructed and immediately before checkpoint
   commit.

An interruption never commits the page checkpoint. If interruption occurs after one
or more mutations were already persisted, the next run scans the same page again.
That replay is intentionally safe only because mutation persistence already owns
inbox deduplication and monotonic source-version guards.

A completed null-cursor checkpoint remains authoritative and returns
`AlreadyComplete` without consulting the probe or source.

## Ownership boundaries

This slice is database and runtime neutral. It does not:

- read `index_jobs` or interpret `cancel_requested`;
- terminalize a replay job as cancelled;
- create a PostgreSQL cancellation probe;
- interrupt an owner future while one source scan or mutation call is currently
  pending;
- add a deadline, timer, polling loop, scheduler, task, or shutdown owner;
- change source, mutation, checkpoint, cursor, event identity, or storage schemas.

The PostgreSQL runner must later bind the probe to the exact active
`(tenant_id, job_id, worker_id, attempt_count)` lease and terminalize a committed
cancel request using its existing fenced cancellation path. That integration must
also classify probe storage failure without exposing raw causes.

The separate production source-call timeout slice bounds selected owner source
futures. Cooperative boundaries and call deadlines are complementary: this worker
contract alone cannot preempt one indefinitely pending source or mutation future.
The combined implementation-plan item for in-page interruption/timeouts therefore
remains open.

## Source evidence

Focused unit scenarios retain:

- interruption before source scan with no source, mutation, or checkpoint write;
- interruption before checkpoint commit after one durable mutation, followed by a
  replay of the same event and duplicate-safe checkpoint completion;
- bounded retryable interruption-probe failure before source access.

Cargo checks, tests, the JavaScript verifier, PostgreSQL execution, cancellation
races, and restart evidence remain maintainer-run and were not executed by the
implementation agent.
