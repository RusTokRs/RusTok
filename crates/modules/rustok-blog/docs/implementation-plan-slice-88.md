# rustok-blog implementation plan — slice 88 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-87.md`.

Slices 1–87 retain the typed Comments remote boundary, delegated-write signing,
scheduled key lifecycle, authorized schedule mutation, canonical schedule
digest persistence, PostgreSQL state/audit atomicity, isolated fault and
fail-stop evidence, and the bounded host admission/writer-port contract for a
future canonical audit publication.

## 2026-08-03 continuation audit

Slice 87 deliberately blocked a canonical writer until `rustok-events` owned a
sealed, registered Blog Comments schedule-audit family and the committed release
digests were generated rather than guessed.

The repository audit still confirms:

- `rustok-events` owns platform event types, typed payload families, schema
  registration, transport envelopes, and release digests;
- `rustok-outbox` already decodes registered typed envelopes and owns canonical
  `sys_events` persistence and relay behavior;
- the Blog-local successful-replacement row already contains the bounded source
  facts required by the event;
- the server-side slice-87 publication document retains the exact request UUID
  as the handoff idempotency identity but intentionally has no writer
  implementation.

## Slice 88 — sealed Blog Comments schedule-audit event contract

### Event identity

The new sealed event family is:

```text
BlogCommentsDelegationScheduleAuditEvent
```

Its only v1 event type is:

```text
blog.comments_delegation_schedule.replacement_succeeded
```

The schema version is `1` and the fixed source state key is:

```text
comments_tcp_delegation_schedule
```

The family is implemented in:

```text
crates/rustok-events/src/blog_comments_schedule_audit.rs
```

It is registered and re-exported through:

```text
crates/rustok-events/src/lib.rs
crates/rustok-events/src/contract.rs
```

### Payload

The v1 payload contains only:

- `audit_schema_version`;
- exact non-nil source `request_id`;
- fixed `state_key`;
- positive `occurred_at_unix_ms`;
- bounded `principal_kind`;
- bounded `operation`;
- bounded `source`;
- positive `previous_generation`;
- strictly greater `candidate_generation`.

Allowed values remain closed:

```text
principal_kind = direct_user | service
operation      = reload_file | replace_host_schedule
source         = host_provided | file
```

The event rejects delegated users indirectly by accepting no
`delegated_user` payload value. It also rejects nil request identities, zero or
out-of-range timestamps, unsupported categories, a non-canonical state key, and
non-increasing generations.

### Request identity exception

Most shared domain facts avoid command or idempotency keys in their payloads.
This audit event is intentionally different: `request_id` is the immutable
identity of the already durable Blog audit fact and is required by the slice-87
canonical writer contract to detect exact replay versus mismatched reuse.

The event does not generate a second request identity and does not use the
canonical envelope UUID as a substitute for the source audit identity.

### Envelope ownership

Control-plane tenant and actor identity remain `ContractEventEnvelope` metadata.
They are not duplicated into the payload.

The payload does not contain:

- delegation key IDs or secrets;
- schedule documents or schedule digests;
- retained/revoked key sets;
- file paths, database URLs, credentials, tokens, or nonces;
- claims, roles, permissions, or authorizer internals;
- raw database errors or free-form operator text.

### Transport registration

`ContractEventPayload` gains the sealed family variant:

```text
blog_comments_delegation_schedule_audit
```

The ordinary typed-envelope path now:

1. validates the bounded Blog audit event;
2. creates a `ContractEventEnvelope` with non-nil tenant metadata;
3. serializes and deserializes the registered family;
4. validates registry and payload metadata after decoding;
5. retains the exact source `request_id`.

Focused source coverage is in:

```text
crates/rustok-events/tests/blog_comments_schedule_audit.rs
```

### Release digest generation

The repository generator was executed once on the temporary technical branch:

```bash
cargo run -p rustok-events --example event_contract_digests
```

It completed successfully with Rust `1.96.0` on the GitHub-hosted
`macos-15-arm64` runner. This command compiled the event crate and printed the
canonical digest artifact; it did not run tests or verifiers.

Generated values:

```text
registry          sha256:add56c12537c74f1c0a41cb7aa36847065eb9747f3443eacc4a8da08f34f4ce7
root_event        sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87
root_envelope     sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d
contract_payload  sha256:4d3f53da292abe8777ff6463941072e3098e22a5d61b44e21c32f40432f590ea
contract_envelope sha256:59a4348d04ce4aa140a974929dbfc28888d0c5784dd0c057e5b6e17b2106d540
```

The unchanged root digests prove that slice 88 adds only the sealed typed family;
it does not change established root event or root envelope wire formats.

The committed artifact is:

```text
crates/rustok-events/contracts/event-contract-digests.json
```

### Preserved ownership

Slice 88 does not modify:

- the Blog PostgreSQL state/audit transaction;
- the Blog audit table or its migration;
- the slice-87 server admission document or writer port;
- `rustok-outbox` transport, relay, lease, retry, DLQ, or retention logic;
- server startup, listeners, workers, manifests, features, or environment
  configuration;
- Comments TCP signing, verification, channel, replay, or authorization behavior.

### Explicit non-claims

Slice 88 does not claim:

- a canonical writer implementation;
- a `sys_events` row written from a Blog audit row;
- exact replay or mismatched-reuse PostgreSQL execution;
- an atomic Blog-row-to-canonical-outbox handoff;
- stored canonical envelope identity or publication fencing;
- a Blog claim query or handoff worker;
- canonical relay execution, retries, DLQ transition, or retention;
- Rust unit/integration test execution;
- JavaScript verifier execution;
- PostgreSQL, migration, workflow-suite, runtime, or production evidence.

Status: `generated_contract_source_ready_maintainer_tests_pending`.
Generator status: `completed_successfully`.
Test policy: `not_run_by_request`.

## Next implementation results

1. Implement `CommentsTcpDelegationScheduleAuditCanonicalWriter` through the
   canonical typed outbox API and return the exact envelope UUID.
2. Add durable exact-request idempotency so exact replay returns the existing
   envelope UUID and mismatched reuse returns `Conflict`.
3. Extend the Blog audit table with canonical envelope identity and bounded
   handoff fencing state.
4. Add one PostgreSQL transaction that locks an unpublished Blog audit row,
   calls the canonical writer, and records the returned envelope UUID with
   `published_at`.
5. Add bounded `FOR UPDATE SKIP LOCKED` source-row claiming without duplicating
   `rustok-outbox` relay ownership.
6. Add ambiguity, restart, stale-claim, concurrent-worker, canonical relay, and
   source-row retention/recovery evidence.

## Suggested maintainer verification — intentionally not run

```bash
cargo test -p rustok-events --test blog_comments_schedule_audit -- --nocapture
cargo test -p rustok-events blog_comments_schedule_audit -- --nocapture
cargo run -p rustok-events --example event_contract_digests
node scripts/verify/verify-blog-comments-audit-event-contract.mjs
```

## Ownership retained

- Blog owns the successful schedule-replacement audit fact and payload semantics.
- The server host owns the control-plane tenant choice and future handoff
  composition.
- `rustok-events` owns the sealed family, registry, envelope wire, and release
  digests.
- `rustok-outbox` owns canonical persistence, claims, relay, retry, fencing,
  DLQ, and canonical-event retention.
- Maintainers own test, verifier, database, workflow, runtime, and production
  validation.
