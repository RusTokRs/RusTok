# rustok-blog implementation plan — slice 94 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-93.md`.

Slices 90–93 retain atomic canonical admission, one host-owned handoff worker,
durable source retry/exhaustion state, source dead-lettering, retry-aware claiming,
and a bounded crash-gap exhaustion sweep.

## 2026-08-03 continuation audit

A fresh audit of `main@3a7ed26b14248c587f208778b854cf2451db3206`
confirmed that the worker now creates source dead letters but operators still had
no request-bound recovery boundary:

- `PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy` exposed only a
  storage-level bounded dead-letter inspection;
- inspection did not bind a request tenant, actor, or effective permission;
- no recovery epoch fenced an operator decision against a later recovery;
- no actor/reason audit fact existed for source requeue;
- no guarded capability was published in `ServerRuntimeContext`;
- GraphQL, HTTP, CLI, MCP, and admin transports remained absent;
- `rustok-outbox` independently retained canonical relay/retry/DLQ ownership.

The same audit found the established server request-bound authorization pattern:
an exact tenant/actor context, `permissions_for`, effective
`Permission::MODULES_MANAGE`, and authorization before adapter validation or
database access.

## Slice 94 — request-bound inspection and audited source requeue

### Host-wide source, exact control-plane authority

The Blog Comments schedule audit source row is host-wide. It is not retrofitted
with a caller-selected tenant owner.

The existing canonical handoff configuration already supplies one mandatory
control-plane tenant:

```text
RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID
```

`CommentsTcpDelegationScheduleAuditOperatorContext` binds one non-nil request
tenant and actor. Every operator call requires the context tenant to equal the
configured control-plane tenant before request validation or storage access.

The boundary then loads the exact request-scoped permission snapshot for the
same tenant and actor and requires effective `modules:manage`.

Closed authorization failures are:

```text
InvalidContext
TenantMismatch
MissingRequestAuthority
Forbidden
```

The operator APIs accept no separate tenant or actor argument. Storage requests
receive tenant and actor only from the authorized context.

### Bounded inspection

The guarded runtime exposes:

```text
inspect_dead_letter(context, request_id)
```

A successful exact terminal inspection returns only:

```text
request_id
attempt_count
recovery_epoch
optional last_failure_code
reason = attempt_budget_exhausted
```

It does not return source payload, schedule generations, original actor,
principal, claim token, timestamps, canonical envelope, SQL, database causes, or
raw diagnostics.

Unknown or non-dead-letter rows return no inspection. Invalid stored terminal
state fails closed.

### Recovery epoch and immutable audit ledger

Migration
`m20260803_000011_create_blog_comments_audit_recovery` adds:

```text
handoff_recovery_epoch BIGINT NOT NULL DEFAULT 0
```

to the Blog source audit table with a nonnegative constraint.

It also creates:

```text
blog_comments_tcp_delegation_schedule_audit_recovery_audits
```

Each immutable audit fact contains:

```text
audit_id
control_plane_tenant_id
request_id
actor_id
action = requeue
reason
prior_attempt_count
recovery_epoch
created_at
```

The reason is explicit, trimmed, non-empty, free of control characters in the
PostgreSQL contract, and bounded to 512 UTF-8 bytes. The application enforces the
same closed reason contract before storage delegation.

`(request_id, recovery_epoch)` is unique. PostgreSQL and SQLite install
`BEFORE UPDATE` and `BEFORE DELETE` rejection triggers. The migration is
intentionally irreversible because deleting recovery epochs or audit facts could
invalidate stale-operator fencing and accountability.

### Exact terminal requeue fence

The guarded runtime exposes:

```text
requeue_dead_letter(
    context,
    request_id,
    expected_attempt_count,
    expected_recovery_epoch,
    reason,
)
```

Authorization occurs before construction of the storage request.

`PostgresCommentsTcpDelegationScheduleAuditRecoveryStore` opens one PostgreSQL
transaction and selects the exact source row `FOR UPDATE`. The row must be:

- unpublished;
- without canonical envelope identity;
- without claim token or claim expiry;
- without a deferred retry timestamp;
- source-dead-lettered with `attempt_budget_exhausted`;
- at a positive attempt count;
- at a nonnegative recovery epoch.

The caller's inspected attempt count and recovery epoch must exactly equal the
locked row. A mismatch returns `StaleInspection` and performs no mutation.

The fenced update repeats:

```text
request_id
handoff_attempt_count
handoff_recovery_epoch
unpublished/noncanonical state
unclaimed state
no deferred retry
terminal attempt_budget_exhausted state
```

### Atomic reset

For one exact terminal row, the same transaction:

1. increments `handoff_recovery_epoch`;
2. resets `handoff_attempt_count` to zero;
3. clears source claim and expiry fields;
4. clears deferred retry and last-failure fields;
5. clears source dead-letter fields;
6. appends one immutable actor/reason recovery audit fact.

The existing retry-aware worker can then claim the row as a fresh first attempt
inside the new recovery epoch. The source request ID and canonical envelope ID
contract remain unchanged; requeue does not create a replacement source row or
canonical event.

The source reset and audit insert commit or roll back together. An ambiguous
commit acknowledgement is reconciled by exact audit ID and exact tenant,
request, actor, reason, prior-attempt, and recovery-epoch facts.

Closed non-error outcomes are:

```text
NotFound
NotDeadLetter
StaleInspection
Requeued
```

### Single startup and runtime ownership

The existing bootstrap continues to call exactly one public symbol:

```text
start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled
```

The symbol now aliases one wrapper that:

1. parses the existing handoff configuration;
2. installs one task-free
   `CommentsTcpDelegationScheduleAuditOperatorRuntime` in
   `ServerRuntimeContext` if enabled;
3. delegates to the existing source-retry handoff worker startup.

The worker lifecycle reservation, task, typed handle, and shared `StopHandle`
remain singular. The operator starts no task and performs no database I/O during
materialization.

### Preserved ownership

Slice 94 does not modify:

- schedule replacement or source audit insertion;
- canonical event type, schema version, payload, or digest;
- canonical write-once writer or source/publication atomic transaction;
- retry-aware worker cycle, attempt budget, retry delay, or exhaustion sweep;
- canonical `sys_events` schema, relay, retry, DLQ, or retention;
- Comments listener, authorization, signing, verification, replay, key, or
  channel behavior;
- module manifests or dependency topology.

Blog owns source recovery state and the immutable source recovery ledger.
The server owns request-bound authorization and guarded capability publication.
`rustok-outbox` remains the sole canonical delivery owner.

### Explicit non-claims

Slice 94 does not claim:

- GraphQL, HTTP, CLI, MCP, or admin transport publication;
- bulk listing, search, pagination, or cross-row recovery;
- automatic requeue or a second retry scheduler;
- recovery audit retention or cleanup;
- PostgreSQL migration, authorization, inspection, requeue, race, restart, or
  ambiguous-commit execution evidence;
- multi-worker retry-ready claim evidence;
- canonical `OutboxRelay` delivery/retry/DLQ evidence;
- Cargo check, formatting, Clippy, Rust tests, JavaScript verifier, workflows,
  runtime, or production validation.

Status: `source_dead_letter_operator_recovery_ready_maintainer_execution_pending`.
Test policy: `not_run_by_request`.
Verifier policy: `not_run_by_request`.

## Next implementation results

1. Retain PostgreSQL evidence for authorization-before-storage, exact inspection,
   successful requeue, stale inspection, non-terminal denial, and audit atomicity.
2. Prove concurrent operator requeues admit at most one recovery epoch.
3. Prove the retry-aware worker claims the requeued row once and starts attempt
   one in the new epoch.
4. Prove deferred timing, attempt exhaustion, stale-worker fencing, restart, and
   ambiguous publication/recovery commits.
5. Prove canonical `OutboxRelay` delivery/retry/DLQ independently from Blog
   source recovery.
6. Define source and recovery-audit retention independently from canonical
   `sys_events` retention.
7. Add a transport only after its error/privacy contract is explicitly sealed.

## Suggested maintainer verification — intentionally not run

```bash
cargo check -p rustok-blog --all-targets --locked
cargo check -p rustok-server --no-default-features --features mod-comments --locked
cargo test -p rustok-server --features mod-comments \
  comments_provider_runtime::keyring_schedule_audit_operator \
  -- --nocapture
node scripts/verify/verify-blog-comments-audit-operator-requeue.mjs
```

## Ownership retained

- Blog owns immutable source rows, source retry/dead-letter state, recovery epoch,
  and append-only recovery facts.
- The server owns control-plane tenant binding, request-bound actor authority,
  `modules:manage` enforcement, and guarded capability publication.
- `rustok-events` owns the sealed canonical event contract.
- `rustok-outbox` owns canonical `sys_events`, relay, retry, DLQ, and retention.
- Maintainers own build, test, verifier, migration, PostgreSQL, runtime, and
  production validation.
