# rustok-blog implementation plan — slice 93 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-92.md`.

Slices 90–92 retain the atomic PostgreSQL source-to-canonical handoff, the
single opt-in host worker, and the durable Blog-owned source retry/dead-letter
policy. Slice 92 intentionally stopped before runtime composition: the worker
still called `publish_next()`, claim selection did not inspect retry/dead-letter
metadata, and policy transitions were available only through explicit storage
calls.

## 2026-08-03 continuation audit

A fresh audit of `main@33783d92a327a8f714141ead9206cce9e8c88bf2`
confirmed:

- the canonical bootstrap mounted exactly one Comments audit handoff startup
  symbol;
- the slice-91 worker owned one lifecycle reservation, one spawned task, one
  shared `StopHandle`, bounded calls per cycle, and deterministic loop delays;
- `publish_next()` hid the exact source claim when publication failed, so the
  slice-92 policy could not safely record the request/token/attempt fence;
- the original slice-90 claim query could select deferred rows, terminal source
  dead letters, or rows already at the configured attempt budget;
- the slice-92 owner exposed durable retry scheduling, exhaustion dead-lettering,
  one-row crash-gap sweeping, and bounded storage inspection, but no runtime
  called those APIs;
- canonical delivery after source acknowledgement remained wholly owned by
  `rustok-outbox`.

## Slice 93 — compose durable source retry into the existing worker

### Compatibility-preserving module extension

Slice 93 does not replace or fork the server bootstrap path. The existing public
startup name remains:

```text
start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled
```

`comments_provider_runtime` now aliases that name to the source-retry-aware
implementation inside the same private worker module. The existing bootstrap
call is unchanged, so there is still exactly one lifecycle reservation, one
worker task, one typed handle, and one shared shutdown subscription.

The slice-90 owner and slice-91 worker source files remain present for retained
compatibility and source history. Retry-aware behavior is added through internal
module extensions that can access the same private claim and lifecycle types;
there is no second relay, second claim table, second task, or independent
shutdown channel.

### Additional strict configuration

The handoff worker remains disabled unless the slice-91 enable flag is exactly
`true`. When enabled, slice 93 adds:

```text
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS=8
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS=30
```

Bounds are:

```text
source max attempts: 1..=100
source retry delay: 1..=86400 whole seconds
```

The source retry delay is durable row state and is distinct from
`RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_RETRY_DELAY_MS`, which remains the
bounded in-memory delay after a worker cycle reports a closed error.

The same strict parsing rules are retained:

- no surrounding whitespace;
- valid UTF-8 only;
- positive decimal integers only;
- values outside the documented bounds fail startup closed.

Defaults are applied only after the handoff worker has been explicitly enabled.
Disabled configuration creates no database owner and no task.

### Retry-aware source claim

The worker uses:

```text
claim_next_retry_ready(source_max_attempts)
```

instead of the compatibility `claim_next()`/`publish_next()` path.

The retry-aware PostgreSQL candidate is eligible only when all of the following
are true:

- `published_at IS NULL`;
- `canonical_envelope_id IS NULL`;
- `handoff_dead_lettered_at IS NULL`;
- `handoff_attempt_count < source_max_attempts`;
- `handoff_next_attempt_at IS NULL OR handoff_next_attempt_at <= NOW()`;
- no claim exists or the previous claim has expired.

Selection remains oldest-first by `created_at, request_id`, uses
`FOR UPDATE SKIP LOCKED`, and claims at most one row.

The same update that installs the random claim token and bounded expiry also
sets `handoff_next_attempt_at = NULL` and increments the durable attempt count.
This preserves the slice-92 database rule that a deferred retry row must be
unclaimed.

Rows at or above the attempt budget are never reclaimed by this worker path.
They remain available only to the bounded exhaustion sweep.

### Explicit claim and publication phases

Each bounded worker iteration now performs:

```text
claim_next_retry_ready(...)
publish_claimed(claim)
```

The worker therefore retains the exact request ID, random claim token, and
attempt count when publication fails. It does not log those identities.

A failure returned before a claim is obtained cannot be durably attributed to a
source row and therefore only affects bounded cycle counters and the in-memory
loop delay.

A failure returned by `publish_claimed(claim)` is passed to:

```text
record_active_failure(claim, error)
```

The storage transition repeats all of these predicates:

- exact request ID;
- unpublished and non-canonical source state;
- no existing source dead letter;
- exact claim token;
- exact durable attempt count;
- `handoff_claim_expires_at > NOW()`.

An expired or replaced claim returns `StaleClaim` without scheduling retry or
creating a source dead letter. The worker ends that cycle so it cannot repeatedly
reclaim the same stale row in a tight loop.

Below the attempt budget, the active transition clears the claim and stores the
configured `handoff_next_attempt_at`. At the budget, it clears the claim and
writes the terminal `attempt_budget_exhausted` source dead letter.

The slice-92 compatibility `record_failure` API remains available to explicit
storage callers, but the canonical runtime lane uses only the stricter
active-expiry entry point.

### One bounded crash-gap sweep

Before each claim batch, the worker calls
`dead_letter_next_expired_exhausted()` at most once.

The sweep continues to:

- select at most one oldest source row;
- require `handoff_attempt_count >= source_max_attempts`;
- require no active claim;
- use `FOR UPDATE SKIP LOCKED`;
- clear claim/retry metadata and write the terminal source dead letter.

This closes the process-loss gap where the final claim reached the budget but
the process stopped before it could record the final failure. Multiple exhausted
rows are drained across bounded cycles rather than through an unbounded inner
loop.

### Bounded cycle outcomes

The source-retry-aware worker records only machine counters:

```text
calls
claimed
published
conflicts
unavailable
retries_scheduled
dead_lettered
stale_claims
swept_dead_letters
policy_invalid_state
policy_unavailable
reached_empty
```

No request, actor, tenant, claim token, envelope, payload, SQL, or raw database
error is logged by the worker layer.

Cycle delay remains deterministic:

- any handoff error, stale claim, or policy error uses the bounded loop retry
  delay in milliseconds;
- an empty ready source uses the idle poll;
- a full successful batch yields for one millisecond before the next cycle.

Policy unavailability or invalid state fails the current cycle closed. A stale
claim also ends the current cycle. Successful retry/dead-letter transitions may
continue to the next ready row until the configured batch bound is reached.

### Cooperative shutdown and transaction ownership

The shared `StopHandle` check remains before each cycle and during the following
sleep. The worker does not cancel an in-flight `claim_next_retry_ready`,
`publish_claimed`, `record_active_failure`, or exhaustion-sweep database call.
Each bounded call finishes or fails closed before the task observes shutdown.

The canonical writer and source published acknowledgement remain in the same
slice-90 PostgreSQL transaction. Source retry/dead-letter transitions occur only
after that publication transaction returns an error.

### Preserved ownership

Slice 93 does not modify:

- the schedule replacement and immutable source-audit insertion transaction;
- the canonical Blog event type, payload, schema version, or digest;
- canonical write-once comparison and exact envelope identity;
- `sys_events` schema;
- `OutboxRelay`, canonical delivery claims, retry, DLQ, or retention;
- Comments authorization, signing, replay, channel, keyring, or listener paths;
- module manifests or dependency topology;
- the single application bootstrap call site.

Blog owns source claim, retry, exhaustion, and source dead-letter state. The
server owns one opt-in bounded task and cooperative shutdown. `rustok-outbox`
remains the sole canonical delivery owner after transaction commit.

### Explicit non-claims

Slice 93 does not add or prove:

- an operator authorization wrapper for source dead-letter inspection;
- actor/reason-audited source requeue or retry-epoch reset;
- HTTP, GraphQL, CLI, MCP, or admin recovery transport;
- exponential backoff, jitter, per-error retry classes, or dynamic budgets;
- source health/readiness or panic supervision;
- PostgreSQL multi-worker, stale-takeover, restart, or ambiguity execution;
- canonical relay delivery/retry/DLQ execution evidence;
- source-audit retention or cleanup;
- Cargo check, formatting, Clippy, Rust tests, JavaScript verifier, migration
  apply, workflows, runtime, or production validation.

Status: `source_retry_runner_composed_maintainer_execution_pending`.
Test policy: `not_run_by_request`.
Verifier policy: `not_run_by_request`.

## Next implementation results

1. Add a server-owned authorization boundary for exact source dead-letter
   inspection.
2. Add actor/reason-audited operator requeue under an exact terminal-row fence,
   without resetting immutable attempt history silently.
3. Prove two-worker retry-aware `SKIP LOCKED` claiming and batch fairness against
   PostgreSQL.
4. Prove deferred retry invisibility, due-time admission, exhaustion exclusion,
   stale-worker no-op, and crash-gap sweeping.
5. Prove process restart, claim-commit ambiguity, publication-commit ambiguity,
   exact replay, and canonical writer conflict.
6. Prove canonical `OutboxRelay` delivery/retry/DLQ independently from Blog
   source recovery.
7. Define source-audit and source dead-letter retention independently from
   canonical `sys_events` retention.

## Suggested maintainer verification — intentionally not run

```bash
cargo check -p rustok-server --no-default-features --features mod-comments --locked
cargo test -p rustok-server --features mod-comments \
  comments_provider_runtime::keyring_schedule_audit_handoff_worker \
  -- --nocapture
node scripts/verify/verify-blog-comments-audit-source-retry-runner.mjs
```

## Ownership retained

- Blog owns immutable source facts and durable source retry/dead-letter state.
- The server owns strict configuration, one worker task, bounded cycle ordering,
  and cooperative shutdown.
- `rustok-events` owns the sealed canonical event contract.
- `rustok-outbox` owns canonical `sys_events`, relay, retry, DLQ, and retention.
- Maintainers own build, test, verifier, PostgreSQL, workflow, runtime, and
  production validation.
