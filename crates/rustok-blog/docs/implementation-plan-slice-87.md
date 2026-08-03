# rustok-blog implementation plan — slice 87 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-86.md`.

Slices 1–86 retain the typed Comments remote boundary, signed user-write
delegation, scheduled key lifecycle, explicitly authorized mutation, canonical
schedule persistence digest, PostgreSQL state CAS, one transactionally coupled
successful-authorization audit row, isolated PostgreSQL success/conflict/fault
harnesses, and the source-ready audited-worker response-disconnect fail-stop
gate.

## 2026-08-03 continuation audit

Slice 86 listed a Blog outbox dispatcher with claim leases, stable request
identity, retry/backoff, publication fencing, and retention as the next source
item.

A fresh repository audit changes the ownership interpretation of that item:

- `rustok-outbox` already owns the canonical `sys_events` table;
- `OutboxRelay` already owns bounded claims, stale-claim recovery, PostgreSQL
  `SKIP LOCKED`, retry/backoff, maximum attempts, DLQ transition, and claim
  fencing;
- the `rustok-outbox` implementation plan explicitly forbids modules and the
  server from reimplementing relay, retry, or DLQ behavior;
- the Blog audit table currently contains a successful authorization fact and a
  `published_at` reservation, but no canonical event-envelope identity or
  handoff ownership contract.

The next Blog responsibility is therefore not a second dispatcher. It is a
bounded handoff contract that lets a future PostgreSQL adapter atomically turn
one existing Blog audit row into one canonical platform event inside the
adapter's transaction.

## Slice 87 — canonical audit handoff admission contract

### Artifact

The source contract is:

```text
apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_publication.rs
```

It is included and re-exported by:

```text
apps/server/src/services/comments_provider_runtime.rs
```

No migration, manifest, feature, dependency, `Cargo.lock`, startup, listener,
worker, or environment configuration is changed.

### Publication document

`CommentsTcpDelegationScheduleAuditCanonicalPublication` contains only bounded
facts already accepted by the audited replacement flow plus one explicit host
scope:

- non-nil host-provided control-plane tenant UUID;
- non-nil request UUID;
- non-nil actor UUID;
- typed `AuthPrincipalKind`;
- typed schedule operation;
- typed keyring source category;
- positive host timestamp in Unix milliseconds;
- positive previous generation;
- strictly greater candidate generation.

The document exports stable closed metadata for a future sealed platform event:

```text
blog.comments_delegation_schedule.replacement_succeeded
schema version 1
comments_tcp_delegation_schedule
```

The document exposes the existing request UUID as `idempotency_key()`. It does
not generate or guess a second Blog identity.

The document intentionally contains no key ID, secret, schedule document,
schedule digest, file path, database URL, credentials, token, nonce, claims,
roles, arbitrary authorizer metadata, raw database error, or free-form operator
text.

### Control-plane tenant ownership

The audited schedule is host-global and the existing trigger context does not
contain tenant scope. Canonical event envelopes reject a nil tenant UUID.

Slice 87 therefore requires a non-nil control-plane tenant UUID supplied by the
host constructing the handoff adapter. It does not:

- invent a reserved UUID;
- read an environment variable;
- use the actor or request UUID as tenant scope;
- copy a tenant from an unrelated user request;
- fall back to `Uuid::nil()`.

Choosing and provisioning the control-plane tenant remains an explicit host and
operator responsibility.

### Principal and generation admission

Only already-admitted direct-user and service principals can form a canonical
publication document. Delegated users remain rejected before writer invocation.

The publication document requires:

```text
previous_generation >= 1
candidate_generation > previous_generation
occurred_at_unix_ms >= 1
```

It preserves the exact typed operation and source category from the durable
Blog row. There is no stringly typed constructor.

### Transaction writer port

`CommentsTcpDelegationScheduleAuditCanonicalWriter` is an async object-safe
transaction boundary:

```text
write_once_in_transaction(
    &DatabaseTransaction,
    &CommentsTcpDelegationScheduleAuditCanonicalPublication,
) -> Result<Uuid, CommentsTcpDelegationScheduleAuditCanonicalWriteError>
```

The caller owns the transaction. A conforming implementation must:

1. use `publication.request_id()` as the stable idempotency identity;
2. write one registered canonical platform event through the canonical outbox
   owner inside the supplied transaction;
3. return the exact canonical envelope UUID written by that transaction;
4. return the existing envelope UUID for an exact duplicate when supported;
5. return `Conflict` when the request UUID is reused for different facts;
6. return `Unavailable` for infrastructure or unreadable-state failures;
7. perform no direct transport publication before commit.

The closed error surface contains only `Conflict` and `Unavailable`. Raw
infrastructure details remain private to the adapter.

### Release-contract boundary

Slice 87 does not add a new `rustok-events` family and does not edit the
committed event-contract digest artifact.

A canonical writer implementation is blocked until a separately reviewed event
contract slice:

- adds the sealed Blog Comments schedule-audit family;
- registers its schema;
- updates the committed registry/payload/envelope digests using the repository
  generator;
- adds transport decoding and compatibility evidence.

This separation avoids publishing an unversioned wire contract or guessing
release digests in a connector-only work unit.

### Canonical relay ownership

A future writer and handoff adapter must delegate delivery to
`rustok-outbox::OutboxRelay`. Blog and the server must not add:

- another `SKIP LOCKED` relay over `sys_events`;
- another claim TTL or worker lease model;
- another retry/backoff counter;
- another DLQ table or status machine;
- direct Iggy, broker, webhook, or HTTP publication;
- a second retention implementation for canonical events.

The Blog-specific handoff may claim its own unpublished audit rows only long
enough to atomically write the canonical event and record that canonical
identity. Delivery after canonical commit belongs to `rustok-outbox`.

### Preserved behavior

Slice 87 does not change:

- schedule authorization or delegated-principal admission;
- PostgreSQL state/outbox SQL and transaction ordering;
- commit reconciliation or fail-stop policy;
- `published_at` semantics;
- the slice-84 and slice-85 PostgreSQL harnesses;
- the slice-86 response-disconnect harness;
- canonical outbox relay logic;
- the public `rustok-events` registry, payload, envelope, or digest artifact;
- TCP transport, listener, channel, signing, verification, or replay behavior.

### Explicit non-claims

Slice 87 does not claim:

- a registered platform event wire contract;
- a canonical `sys_events` write implementation;
- a Blog audit-row claim query;
- an atomic Blog-row-to-`sys_events` handoff;
- a stored canonical envelope UUID;
- publication fencing or ambiguous-commit reconciliation;
- dispatcher, relay, retry, DLQ, or retention execution;
- PostgreSQL, migration, transaction, or concurrency execution;
- Rust compilation or test execution;
- JavaScript verifier execution;
- workflow, CI, runtime, or production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add the sealed `rustok-events` Blog Comments schedule-audit family and update
   the committed event-contract digest artifact with the repository generator.
2. Implement the canonical writer through `rustok-outbox` and prove exact
   request-id idempotency plus mismatched-reuse conflict behavior.
3. Add a bounded PostgreSQL handoff adapter over unpublished Blog audit rows,
   using `FOR UPDATE SKIP LOCKED` and the writer in one transaction.
4. Extend the Blog audit table with the canonical envelope UUID and handoff
   fencing state, then atomically record that UUID with `published_at`.
5. Add commit-acknowledgement ambiguity, restart, stale-claim, concurrent-worker,
   canonical-relay retry/DLQ, and cleanup evidence.
6. Define operator recovery and retention policy for the source Blog audit rows
   without duplicating canonical outbox retention.

## Suggested verification — intentionally not run

```bash
node scripts/verify/verify-blog-comments-audit-canonical-handoff-contract.mjs
cargo test -p rustok-server \
  services::comments_provider_runtime::keyring_schedule_audit_publication::tests \
  -- --nocapture
cargo check -p rustok-server --features mod-blog --locked
```

## Ownership retained

- Comments owns schedule lifecycle validation, key selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns authorization, audited trigger composition, PostgreSQL
  state/audit transaction logic, and the control-plane tenant choice.
- Blog owns the successful schedule-replacement audit fact and its future event
  payload semantics.
- `rustok-events` owns sealed platform event families and release digests.
- `rustok-outbox` owns canonical event persistence, claims, relay, retry,
  fencing, DLQ, and canonical-event retention.
- Maintainers own executable validation and operator policy.
