# rustok-blog implementation plan — slice 92 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-91.md`.

Slices 89–91 retain the exact canonical `rustok-outbox` writer, the atomic
PostgreSQL source-to-canonical handoff owner, and one opt-in bounded host runner
with cooperative shutdown.

## 2026-08-03 continuation audit

A fresh audit of `main@0dead879115b89002fb458e736781c566efa40ff`
confirmed that the runner intentionally had no durable source failure policy:

- every claim already increments `handoff_attempt_count` behind a unique claim
  token and bounded expiry;
- failed `publish_next()` calls were counted only in process memory;
- the exact source row had no retry-ready timestamp, bounded failure code, or
  terminal source dead-letter state;
- an expired final claim after process loss had no explicit exhaustion sweep;
- operator inspection/requeue and actor/reason audit remained undefined;
- canonical `sys_events` retry and DLQ remained correctly owned by
  `rustok-outbox` and must not be reused for Blog source recovery.

## Slice 92 — PostgreSQL source retry and dead-letter policy owner

### Irreversible source schema

Migration
`m20260803_000010_add_blog_comments_audit_source_retry_policy` adds:

```text
handoff_next_attempt_at TIMESTAMPTZ NULL
handoff_last_failure_at TIMESTAMPTZ NULL
handoff_last_failure_code VARCHAR(32) NULL
handoff_dead_lettered_at TIMESTAMPTZ NULL
handoff_dead_letter_reason VARCHAR(64) NULL
```

The migration is intentionally irreversible. PostgreSQL constraints require:

- last failure timestamp and code to be present or absent together;
- failure code to be exactly `conflict` or `unavailable`;
- dead-letter timestamp and reason to be present or absent together;
- dead-letter reason to be exactly `attempt_budget_exhausted`;
- retry-ready rows to remain unpublished, non-canonical, non-dead-lettered, and
  unclaimed;
- dead-letter rows to remain unpublished, non-canonical, non-retrying, and
  unclaimed;
- published rows to have no retry or dead-letter state.

Separate indexes support future retry-ready selection and bounded dead-letter
inspection. Existing rows retain null policy metadata; the migration does not
guess failures or exhaustion.

### Explicit policy owner

The server publishes:

```text
PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy
```

Construction requires:

- PostgreSQL;
- a source attempt budget in `1..=100`;
- a whole-second retry delay in `1..=86400` seconds.

The owner is task-free and exposes explicit calls only:

```text
record_failure(claim, handoff_error)
dead_letter_next_expired_exhausted()
inspect_dead_letter(request_id)
```

This slice does not construct the owner in bootstrap, register it in
`ServerRuntimeContext`, or change the slice-91 runner.

### Exact fenced failure recording

`record_failure` accepts the slice-90 claim value and one closed handoff error:

```text
Conflict
Unavailable
```

The PostgreSQL update repeats the exact source fence:

```text
request_id
handoff_claim_token
handoff_attempt_count
published_at IS NULL
canonical_envelope_id IS NULL
handoff_dead_lettered_at IS NULL
```

A replaced, published, canonicalized, or already dead-lettered claim returns the
closed `StaleClaim` transition without mutating another worker's state.

For an authoritative failed claim below the configured attempt budget, one
statement:

- stores the bounded failure timestamp and machine code;
- clears the claim token and expiry;
- sets one deterministic retry-ready timestamp from the configured delay;
- returns `RetryScheduled` with only request ID and attempt count.

At or above the attempt budget, the same statement instead clears retry state
and writes the terminal `attempt_budget_exhausted` source dead letter. It returns
`DeadLettered` with only request ID and attempt count.

No raw database error, SQL, payload, actor, tenant, claim token, or timestamp is
returned by the policy API.

### Process-loss exhaustion sweep

A process can disappear after acquiring the final allowed claim but before it
records a failure. `dead_letter_next_expired_exhausted()` closes that specific
gap.

Each explicit call transitions at most one oldest row satisfying:

```text
published_at IS NULL
canonical_envelope_id IS NULL
handoff_dead_lettered_at IS NULL
handoff_attempt_count >= configured max attempts
no claim or expired claim
```

Selection uses `FOR UPDATE SKIP LOCKED`, clears stale claim/retry metadata, and
writes the same terminal dead-letter reason. The call does not loop, poll, sleep,
or spawn a task.

### Bounded storage inspection

`inspect_dead_letter(request_id)` reads one exact terminal source dead letter and
returns only:

```text
request_id
positive handoff_attempt_count
optional closed last_failure_code
attempt_budget_exhausted reason
```

It does not return actor, tenant, operation, source, generations, timestamps,
claim identity, payload, SQL, or raw diagnostic text.

This storage API is not an authorization boundary and is not exposed through
HTTP, GraphQL, CLI, MCP, or admin transports. A later server-owned wrapper must
bind actor and tenant, require the effective management permission, and record
an immutable operator reason before any requeue is admitted.

### Preserved ownership

Slice 92 does not modify:

- the schedule replacement transaction or immutable source audit insertion;
- slice-90 claim selection, claim TTL, canonical writer transaction, terminal
  publication update, or ambiguous-commit reconciliation;
- the slice-91 runner configuration, loop, bootstrap mount, or shutdown path;
- the registered Blog event payload, type, schema version, or digest;
- `sys_events`, canonical write-once comparison, `OutboxRelay`, canonical retry,
  canonical DLQ, or canonical retention;
- Comments authorization, signing, verification, replay, key, channel, or
  listener behavior;
- module manifests or dependency topology.

Blog owns the new source retry/dead-letter metadata. `rustok-outbox` remains the
sole canonical delivery owner.

### Explicit non-claims

Slice 92 does not claim:

- that runner failures automatically call `record_failure`;
- that claim selection honors `handoff_next_attempt_at` or excludes source dead
  letters yet;
- that the exhaustion sweep is scheduled or mounted;
- exponential backoff, jitter, per-error budgets, or automatic requeue;
- authorized operator inspection, actor/reason audit, or operator requeue;
- source-claim heartbeat or extension;
- source-audit retention or cleanup;
- PostgreSQL migration, concurrency, restart, or ambiguous-commit execution;
- Cargo check, formatting, Clippy, Rust tests, JavaScript verifier, workflows,
  runtime, or production validation.

Status: `source_retry_policy_ready_runner_composition_pending`.
Test policy: `not_run_by_request`.
Verifier policy: `not_run_by_request`.

## Next implementation results

1. Compose the policy owner into the slice-91 runner with explicit max-attempt
   and source retry-delay configuration.
2. Make claim selection honor `handoff_next_attempt_at` and exclude source dead
   letters without changing canonical relay ownership.
3. Call the bounded crash-gap exhaustion sweep from the same runner lane.
4. Add a server-owned authorized dead-letter inspection and requeue boundary
   with immutable actor/reason audit facts.
5. Prove two-worker `SKIP LOCKED` fairness, delayed retry eligibility, terminal
   exhaustion, process restart, stale takeover, stale-worker rollback, exact
   replay, writer conflict, and ambiguous commits against PostgreSQL.
6. Prove canonical `OutboxRelay` delivery/retry/DLQ separately.
7. Define source-audit retention independently from canonical retention.

## Suggested maintainer verification — intentionally not run

```bash
cargo check -p rustok-blog --all-targets --locked
cargo check -p rustok-server --no-default-features --features mod-comments --locked
cargo test -p rustok-server --features mod-comments \
  comments_provider_runtime::keyring_schedule_audit_source_retry_postgres \
  -- --nocapture
node scripts/verify/verify-blog-comments-audit-source-retry-policy.mjs
```

## Ownership retained

- Blog owns immutable source audit facts and source retry/dead-letter state.
- The server owns the explicit PostgreSQL policy adapter and future authorized
  composition.
- `rustok-events` owns the sealed canonical event contract.
- `rustok-outbox` owns canonical admission, relay, retry, DLQ, and retention.
- Maintainers own migration, build, test, verifier, PostgreSQL, runtime, and
  production validation.
