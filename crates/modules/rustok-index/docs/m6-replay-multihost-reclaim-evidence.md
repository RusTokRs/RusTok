# M6 replay multi-host reclaim evidence

Status: `source_complete_execution_pending`.

## Purpose

The replay source already retained sequential resume, graceful-stop restart, locale restart and stale-lease unit evidence. The remaining source-only gap was a deterministic two-host ownership race through the real multi-page runner: one host must still be inside a page when another host reclaims the expired durable job, and the late first host must be unable to publish checkpoint or terminal state after ownership changed.

This slice adds retained SQLite source evidence for that exact boundary without changing production replay behavior.

## Retained scenario

`source_replay_multihost_restart_tests.rs` composes two distinct `PostgresIndexReplayRunner` instances over the same database, schema registry and stable replay source.

The packet runs this sequence:

1. host A acquires the schema replay job as attempt 1 and enters the first source scan;
2. the source signals `first_host_scan_started` and blocks only that first scan on `release_first_host_scan`;
3. the evidence fixture deterministically moves host A's persisted `lease_expires_at` into the past instead of waiting on wall-clock expiry;
4. host B invokes the ordinary runner for the same replay scope, reclaims the same durable job as attempt 2 and completes the stable mutation, checkpoint and job;
5. only after host B has succeeded does the packet release host A's original source scan;
6. host A observes the same stable delivery after host B already applied it, then its checkpoint path fails closed on the stale attempt fence as `IndexReplayRunError::LeaseLost`;
7. final durable state remains exactly one succeeded attempt-2 job, one complete checkpoint, one applied inbox delivery and one materialized entity.

The first host therefore cannot overwrite the attempt-2 checkpoint, convert the completed job to failed/pending/cancelled, or create a second durable delivery after reclaim.

## Why the test expires the lease directly

The production lease is deliberately at least 60 seconds and is maintained during long pages. Waiting for real lease expiry would make retained evidence slow and timing-sensitive.

The packet changes only `lease_expires_at` on the already-created running attempt. It does not insert a replay job, insert/update a checkpoint, claim attempt 2, publish success, or call the job store directly. Host B performs reclaim and attempt increment through the real `PostgresIndexReplayRunner` / `PostgresIndexReplayJobStore` path.

This is a deterministic clock-boundary fixture, not a production administrative API.

## Fencing and idempotency contract

Reclaim remains owned by the durable job SQL contract:

- an expired `running` job is claimable;
- the new owner increments `attempt_count` and receives a new lease owner identity;
- heartbeat, checkpoint lease assertion, terminal success/failure, yield and cancellation all require the current owner/attempt fence;
- host A's stale attempt therefore cannot publish after host B has claimed attempt 2.

The mutation path intentionally remains independently idempotent. A stale host can have observed owner data before learning that its job lease was replaced; stable event/delivery identity and the inbox/source-version contract make that late delivery duplicate-safe. The checkpoint fence is the durable replay progress authority.

## Relationship to existing restart evidence

This packet complements rather than replaces the retained restart slices:

- bounded runner evidence already proves a yielded job resumes with the same job id and incremented attempt;
- graceful-stop evidence proves duplicate-safe resume after a durable mutation but before checkpoint commit;
- GraphQL locale evidence rebuilds fresh runtime/operator/GraphQL composition over the same durable database and resumes locale attempt 2;
- this packet adds concurrent ownership replacement: host B completes while host A still holds an in-flight page future, and the late host A is fenced from durable replay progress.

Together those retained sources cover the source-only multi-host/restart boundary currently tracked by the M6 cursor. Production execution/admission remains maintainer-owned.

## Explicit non-goals

This slice does not add or claim:

- automatic retry, requeue or scheduler ownership;
- a second replay job ownership model;
- distributed consensus beyond the existing database lease fence;
- a new heartbeat/cancellation/shutdown contract;
- partition replay scope;
- targeted/full/shadow rebuild modes;
- PostgreSQL execution evidence;
- process/container orchestration evidence.

## Retained validation source

`verify-index-replay-multihost-reclaim-evidence.mjs` locks the packet shape, real runner usage, deterministic lease-expiry seam, attempt-2 completion, stale-host `LeaseLost` result, durable final-state assertions and the absence of direct job/checkpoint creation or retry/scheduler semantics.

The retained Rust test and Node verifier were not executed by the implementation agent.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, SQLite/PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
