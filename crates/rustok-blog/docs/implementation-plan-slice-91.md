# rustok-blog implementation plan — slice 91 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-90.md`.

Slices 87–90 retain the bounded canonical publication port, the sealed Blog
Comments schedule-audit event, the `rustok-outbox` exact write-once writer, and
the PostgreSQL source-row claim/fencing owner that commits canonical admission
and source acknowledgement in one transaction.

## 2026-08-03 continuation audit

A fresh audit of `main@38ec652d92f411fd771f3290bbb2b210c9d5fe26`
confirmed that slice 90 was source-ready but not running:

- `PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff` exposed explicit
  `claim_next`, `publish_claimed`, and `publish_next` calls;
- no host task called the owner;
- the server already had one shared `StopHandle` and established cooperative
  shutdown loops;
- the single application bootstrap path already mounted opt-in Comments and
  other bounded workers;
- no control-plane tenant configuration existed for this canonical audit lane;
- retry exhaustion, source dead-letter, operator requeue, and source retention
  remained undefined and must not be guessed by a runtime loop.

## Slice 91 — host-owned bounded handoff runner

### Opt-in configuration

The runner is disabled by default. It is enabled only with:

```text
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_ENABLED=true
```

When enabled, the following canonical host identity is mandatory:

```text
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID=<canonical non-nil UUID>
```

Bounded optional settings are:

```text
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CLAIM_TTL_SECONDS=60   # 1..=300
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_IDLE_POLL_MS=1000     # 1..=60000
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_RETRY_DELAY_MS=1000   # 1..=60000
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE=32 # 1..=256
```

Configuration parsing is strict:

- booleans are exactly `true` or `false`;
- UUIDs must use canonical lowercase hyphenated text and must be non-nil;
- numeric values must be positive and inside their documented bound;
- surrounding whitespace and non-UTF-8 environment values fail startup closed.

An enabled runner requires a host mode authorized to run background workers and
a PostgreSQL connection. Disabled configuration has no database or task side
effect.

### One lifecycle owner

The server publishes:

```text
CommentsTcpDelegationScheduleAuditHandoffWorkerHandle
```

and reserves startup through a dedicated typed lifecycle reservation in
`ServerRuntimeContext`. A repeated startup call is a no-op after the first owner
wins the reservation. Failed construction releases the reservation so corrected
configuration can be retried by a later explicit bootstrap.

The worker receives the server-wide `StopHandle` subscription. If no stop handle
exists yet, it atomically installs the same type used by the rest of the server.
No independent shutdown channel or abort-only lifecycle is introduced.

The single `bootstrap_application_router` path mounts the runner after Comments
runtime composition and before the generic runtime workers are connected.

### Bounded execution cycle

Each cycle calls the slice-90 `publish_next()` owner no more than the configured
`max_claims_per_cycle`.

The cycle records only bounded machine counts:

```text
calls
published
conflicts
unavailable
reached_empty
```

It does not log request, actor, tenant, claim-token, envelope, payload, SQL, or
database error details.

Cycle delay is deterministic:

- any closed `Conflict` or `Unavailable` result uses the bounded retry delay;
- an empty source scan uses the bounded idle poll;
- a full successful batch yields for one millisecond before the next bounded
  cycle, avoiding an unbounded inner drain loop.

An error does not terminate the task and does not create a second retry state.
The durable source claim/expiry from slice 90 remains authoritative. Active
failed claims are skipped by later scans; expired claims are recovered by the
slice-90 owner.

### Cooperative shutdown

The worker checks the shared stop signal before every cycle. It does not cancel a
`publish_next()` future after the source/canonical transaction has begun. The
current bounded owner call is allowed to finish or fail closed; the following
sleep is interruptible through `tokio::select!` on `StopHandle`.

This preserves the atomic canonical-write plus source-acknowledgement boundary.
No task abort is required for normal shutdown.

A source-claim heartbeat is not added in this slice. During
`publish_claimed()` the exact source row is locked by the PostgreSQL transaction;
a competing `SKIP LOCKED` claimant cannot take that row while the canonical
writer and terminal source update execute. A disconnected transaction rolls
back and leaves the durable claim eligible after its bounded expiry.

### Preserved ownership

Slice 91 does not modify:

- the schedule replacement transaction or source audit insertion;
- slice-90 claim SQL, fencing fields, or terminal source update;
- the registered Blog event payload, type, schema version, or digest;
- `sys_events` schema or canonical write-once comparison;
- `OutboxRelay`, canonical relay claims, retries, DLQ, or retention;
- Comments authorization, signing, verification, replay, key, channel, or
  listener behavior;
- module manifests or dependency topology.

Blog owns only source-row handoff admission and its host runner. `rustok-outbox`
continues to own canonical delivery after transaction commit.

### Explicit non-claims

Slice 91 does not claim:

- source retry budgets, exponential backoff, jitter, or attempt exhaustion;
- source dead-letter state or operator inspection/requeue;
- source claim heartbeat or extension;
- source-audit retention or cleanup;
- health/readiness endpoint integration or supervisor restart after panic;
- multiple-process, stale-takeover, stale-worker, restart, or ambiguous-commit
  PostgreSQL execution evidence;
- canonical `OutboxRelay` delivery/retry/DLQ runtime evidence;
- Cargo check, formatting, Clippy, Rust tests, JavaScript verifier, workflows,
  runtime, or production validation.

Status: `canonical_handoff_runner_source_ready_maintainer_execution_pending`.
Test policy: `not_run_by_request`.
Verifier policy: `not_run_by_request`.

## Next implementation results

1. Add a separate source-row retry/exhaustion/dead-letter schema and policy.
2. Add authorized operator inspection and requeue with actor/reason audit facts.
3. Prove two-worker `SKIP LOCKED` claiming and bounded batch fairness against
   PostgreSQL.
4. Prove stale claim takeover and stale-worker rollback without duplicate
   canonical events.
5. Prove process restart, claim-commit ambiguity, publication-commit ambiguity,
   exact replay, and writer conflict.
6. Prove canonical `OutboxRelay` delivery/retry/DLQ independently from Blog
   source-row recovery.
7. Define source-audit retention independently from canonical `sys_events`
   retention.

## Suggested maintainer verification — intentionally not run

```bash
cargo check -p rustok-server --no-default-features --features mod-comments --locked
cargo test -p rustok-server --features mod-comments \
  comments_provider_runtime::keyring_schedule_audit_handoff_worker \
  -- --nocapture
node scripts/verify/verify-blog-comments-audit-handoff-runner.mjs
```

## Ownership retained

- Blog owns the immutable source audit row and source handoff lifecycle.
- The server owns opt-in worker configuration, control-plane tenant selection,
  task lifecycle, bounded polling, and cooperative shutdown.
- `rustok-events` owns the sealed canonical event contract.
- `rustok-outbox` owns canonical `sys_events`, relay, retry, DLQ, and retention.
- Maintainers own build, test, verifier, PostgreSQL, runtime, and production
  validation.
