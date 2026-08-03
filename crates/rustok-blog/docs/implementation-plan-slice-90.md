# rustok-blog implementation plan — slice 90 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-89.md`.

Slices 83–89 retain the PostgreSQL schedule-state/audit transaction, fail-stop
handling for indeterminate audited writes, the bounded canonical publication
port, the sealed registered Blog Comments schedule-audit event, and the
`rustok-outbox` typed write-once canonical writer.

## 2026-08-03 continuation audit

A fresh audit of `main` found that one ownership gap remained:

- successful schedule replacement already commits the new schedule state and one
  immutable Blog audit row in the same PostgreSQL transaction;
- the Blog audit row already owns the durable non-nil `request_id` used by slice
  89 as the canonical envelope UUID;
- the slice-89 writer can insert or resolve the exact typed `sys_events` row in a
  caller-owned transaction;
- no production path claims an unpublished Blog audit row, invokes that writer,
  and marks the exact source row published in the same transaction;
- the existing nullable `published_at` field has no canonical envelope identity
  or stale-worker fencing metadata;
- `rustok-outbox` already owns canonical relay, retry, claim, DLQ, and retention,
  so the Blog side must only own source-row handoff admission.

## Slice 90 — PostgreSQL atomic source-to-canonical handoff

### Irreversible schema extension

Migration
`m20260803_000009_add_blog_comments_audit_canonical_handoff` adds:

```text
canonical_envelope_id UUID NULL
handoff_claim_token UUID NULL
handoff_claim_expires_at TIMESTAMPTZ NULL
handoff_attempt_count BIGINT NOT NULL DEFAULT 0
```

The migration is intentionally irreversible. PostgreSQL checks require:

- a non-negative handoff attempt count;
- claim token and expiry to be both null or both present;
- every claim token to be non-nil;
- a terminal row to have both `published_at` and
  `canonical_envelope_id = request_id`;
- a published row to have no remaining source claim.

Unique indexes bind one canonical envelope identity and one claim token to at
most one Blog audit row. A bounded pending index supports oldest-first claim
selection.

The migration fails closed if a pre-existing row already uses `published_at`
without the canonical identity introduced here. It does not guess or backfill a
canonical event that cannot be proven.

### Explicit handoff owner

The server publishes:

```text
PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff
```

Construction requires:

- a PostgreSQL `DatabaseConnection`;
- a non-nil host control-plane tenant UUID;
- the slice-87 canonical writer port;
- a whole-second source claim TTL in `1..=300` seconds.

The owner exposes explicit calls only:

```text
claim_next()
publish_claimed(claim)
publish_next()
```

This slice does not register the owner in runtime extensions, spawn a task, add
an environment switch, or start a polling loop.

### Bounded source-row claiming

`claim_next()` opens a short PostgreSQL transaction and selects one oldest
eligible source row using:

```text
FOR UPDATE SKIP LOCKED
```

A row is eligible only when:

- `published_at IS NULL`;
- `canonical_envelope_id IS NULL`;
- no claim exists, or the previous claim has expired.

The same statement writes one random non-nil claim token, a bounded expiry, and
increments the durable attempt count. The returned claim contains only:

```text
request_id
claim_token
attempt_count
```

A claim-commit acknowledgement failure is reconciled by the unique claim token.
No transport or canonical outbox row is touched during claim acquisition.

### Fenced atomic publication

`publish_claimed()` locks the exact Blog audit row with `FOR UPDATE` and validates:

- the request, actor, state, source, operation, generations, outcome, and schema;
- the exact claim token;
- the exact durable attempt count;
- a still-active claim expiry;
- absence of a conflicting canonical/published terminal pair.

It then builds the bounded slice-87 publication and invokes:

```text
CommentsTcpDelegationScheduleAuditCanonicalWriter::write_once_in_transaction
```

inside the same PostgreSQL transaction. The writer must return the exact source
`request_id` as the canonical envelope UUID.

Only after canonical admission succeeds does the same transaction update the
source row to:

```text
canonical_envelope_id = request_id
published_at = NOW()
handoff_claim_token = NULL
handoff_claim_expires_at = NULL
```

The final update repeats the request/token/attempt/active-expiry fence. If a
claim expires or is replaced, both the canonical insert and source update roll
back together.

### Exact replay and ambiguous commit handling

An already-terminal exact source row returns its stored `request_id` without a
second canonical event. A mismatched claim or replaced attempt returns the
closed `Conflict` error. Invalid or unreadable durable state returns the closed
`Unavailable` error.

If the final commit acknowledgement is ambiguous, the owner reads the exact
source row. Success is admitted only when:

```text
published_at IS NOT NULL
canonical_envelope_id = request_id
```

Because the `sys_events` write and source terminal update share one transaction,
that pair is the durable acknowledgement boundary. No separate Blog receipt or
canonical relay lease is introduced.

### Preserved ownership

Slice 90 does not modify:

- the registered Blog event type, payload, schema version, or generated digest;
- the generic slice-89 write-once comparison algorithm;
- `sys_events` schema or migration;
- `OutboxRelay`, canonical claims, retry/backoff, DLQ, or retention;
- schedule replacement authorization, signing, verification, replay, channel,
  or key lifecycle behavior;
- the existing atomic schedule-state plus Blog-audit insert transaction;
- listener startup, runtime manifests, module dependencies, or environment
  configuration.

### Explicit non-claims

Slice 90 does not claim:

- that a host-owned handoff loop is mounted or running;
- heartbeat or claim extension during long publication;
- automatic retry, delay, jitter, exhaustion, or operator requeue policy;
- a dead-letter state for the Blog source row;
- source-row retention or cleanup;
- PostgreSQL migration, concurrency, restart, or ambiguous-commit execution;
- Rust unit or integration test execution;
- JavaScript verifier execution;
- Cargo check, formatting, Clippy, workflow, runtime, or production validation.

Status: `canonical_handoff_source_ready_maintainer_execution_pending`.
Test policy: `not_run_by_request`.
Verifier policy: `not_run_by_request`.

## Next implementation results

1. Compose one host-owned bounded handoff runner from this explicit owner and the
   slice-89 writer.
2. Define idle polling, retry delay, attempt exhaustion, source dead-letter and
   operator recovery policy without reusing canonical outbox relay fields.
3. Add cooperative shutdown and, if publication work can exceed the bounded
   claim TTL, a fenced source-claim heartbeat.
4. Prove concurrent `SKIP LOCKED` claiming, stale claim takeover, stale-worker
   rollback, exact replay, writer conflict, and commit-acknowledgement ambiguity
   against PostgreSQL.
5. Prove restart recovery and canonical `OutboxRelay` delivery/retry/DLQ as a
   separate execution lane.
6. Define Blog source-audit retention independently from canonical `sys_events`
   retention.

## Suggested maintainer verification — intentionally not run

```bash
cargo check -p rustok-blog --all-targets --locked
cargo check -p rustok-server --no-default-features --features mod-blog --locked
cargo test -p rustok-server --features mod-blog \
  comments_provider_runtime::keyring_schedule_audit_handoff_postgres \
  -- --nocapture
node scripts/verify/verify-blog-comments-audit-canonical-handoff-postgres.mjs
```

## Ownership retained

- Blog owns the durable source audit row, source claim, source fencing, terminal
  source acknowledgement, and future source retention policy.
- The server owns control-plane tenant selection and the explicit handoff owner.
- `rustok-events` owns the sealed canonical payload and envelope validation.
- `rustok-outbox` owns canonical `sys_events` admission, relay, retry, DLQ, and
  canonical retention.
- Maintainers own migration, test, verifier, PostgreSQL, runtime, and production
  validation.
