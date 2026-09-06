# rustok-blog implementation plan — slice 83 continuation

This document continues `crates/rustok-blog/docs/implementation-plan-slice-82.md`.

Slices 1–82 retain the typed Comments remote boundary, signed user-write
delegation, scheduled key lifecycle, one process-local replay gate, explicitly
authorized mutation, bounded process-local audit, canonical secret-inclusive
schedule digest, persist-before-publish replacement, and a concrete PostgreSQL
state compare-and-store adapter.

## 2026-08-01 continuation audit

Slice 82 durably commits generation and digest before the new in-memory snapshot
is published, but the authorization identity that caused a successful mutation
remains only in the bounded process-local audit ring. A crash can therefore
retain the accepted schedule generation while losing the local actor/request
record.

Slice 83 adds a separate PostgreSQL-audited trigger profile. For every successful
authorized replacement it writes one typed durable audit/outbox row in the same
PostgreSQL transaction as the exact state CAS.

Tests, source verifiers, formatting, Cargo commands, PostgreSQL execution,
workflows, and CI remain intentionally unexecuted by request.

## Slice 83 — transactional authorization audit outbox

### Separate opt-in profile

`SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger` wraps the unchanged
slice-81 persisted trigger. Construction requires:

- a canonical host document or fixed version-2 schedule file;
- runtime maximum delegation TTL;
- the mandatory slice-80 authorizer;
- `PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore`;
- explicit `BootstrapEmpty` or `ResumeExact` startup mode;
- process-local audit capacity within `1..=1024`.

The runtime guard permits exactly one of:

- PostgreSQL-audited persisted trigger;
- ordinary persisted trigger;
- process-local trigger.

Any trigger plus a standalone schedule handle is rejected.

The ordinary slice-82 PostgreSQL adapter and ordinary slice-81 persisted trigger
remain available and unchanged. Durable audit is therefore explicit rather than
silently changing every persistence implementation.

### One-shot operation context

Before calling the inner persisted trigger, the audited profile installs one
process-local context containing:

- non-nil request UUID;
- non-nil actor UUID;
- typed principal kind;
- closed operation enum;
- host audit timestamp.

An outer operation mutex prevents contexts from overlapping before the inner
trigger acquires its own operation mutex. An RAII guard clears the context after
all normal outcomes, including authorization denial, unavailable authorization,
candidate rejection, state conflict, or success.

The context itself does not authorize the operation. The unchanged inner trigger
still performs delegated-user rejection and mandatory host authorization. Only a
call that reaches the persistence CAS can consume the context.

If execution unwinds after the context is installed, the profile fail-stops the
process rather than risk continuing with an unknown store/snapshot boundary.

### Durable audit/outbox table

The Blog migration
`m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox` creates:

```text
blog_comments_tcp_delegation_schedule_audit_outbox
```

Each row contains:

- audit schema version 1;
- request UUID as primary key and outbox event identity;
- fixed schedule state key;
- fixed event type `comments_tcp_delegation_schedule_replaced`;
- host occurrence time in Unix milliseconds;
- actor UUID;
- principal kind `direct_user` or `service`;
- operation `reload_file` or `replace_host_schedule`;
- source `host_provided` or `file`;
- previous generation;
- candidate generation;
- fixed outcome `replacement_succeeded`;
- database creation time;
- nullable future publication time.

A unique `(state_key, candidate_generation)` index permits at most one durable
success event for each accepted generation. The outbox row references the
singleton schedule state row. The migration is intentionally irreversible.

The row stores no key IDs, secrets, schedule document, digest, file path,
credential, nonce, token, authorization metadata, or raw backend error.

Actor and request UUIDs are intentionally retained because this slice is the
durable authorization audit owner.

### Atomic state and audit commit

For an authorized replacement the audited store performs one PostgreSQL
transaction:

1. predicate-update the singleton state row using the complete expected schema,
   source, generation, and digest;
2. require exactly one affected state row;
3. insert the typed audit/outbox row with `ON CONFLICT DO NOTHING`;
4. require exactly one affected outbox row;
5. commit;
6. only after definitive success allow slice 81 to assign the in-memory snapshot.

A stale state predicate, reused request UUID, or reused candidate generation
returns `Conflict`. The transaction rolls back, so neither state nor outbox is
advanced.

Bootstrap remains a state-only operation. It does not invent an actor or an
authorization event. `ResumeExact` remains read-only.

### Fail-stop write policy

The audited profile is intentionally stricter than the ordinary slice-82
adapter. Any `Unavailable` result on a write path causes `std::process::abort()`.
This includes failures that may have happened before commit; the profile chooses
availability loss rather than risk returning control after an unknown write.

After PostgreSQL reports a commit error, reconciliation repeatedly reads both:

- the singleton state row; and
- the outbox row addressed by request UUID.

Only exact candidate state plus exact audit row is accepted as success. Every
other definitive pair is fail-stop. Unreadable state is retried 20 times with a
100 ms delay and then fail-stops.

A response-channel disconnect after a submitted write becomes `Unavailable` and
therefore fail-stops. If execution unwinds while a write may be in flight, the
outer completion guard also fail-stops.

Consequently the audited profile never returns a normal write error that might
represent a committed state/outbox pair. Definitive CAS conflicts remain normal
errors because the transaction did not commit.

### Process-local audit relationship

The unchanged slice-81 bounded audit ring remains active and records all closed
local outcomes, including denial and failure. The durable outbox records only
successful authorized state transitions.

This slice therefore claims crash-safe retention of successful authorization
identity when used with the audited PostgreSQL profile. It does not claim durable
completeness for:

- delegated-user rejection;
- authorization denial;
- authorization service unavailability;
- preflight or candidate rejection;
- persistence conflict;
- failures before a successful transaction.

### Outbox publication boundary

`published_at` is reserved for a future dispatcher. Slice 83 does not add:

- a polling worker;
- lease/claim ownership;
- delivery retries;
- broker publication;
- retention or archival;
- an external audit API.

The row is append-only from the mutation path. No delete or update owner is added.

### Preserved behavior

Slice 83 does not change:

- canonical schedule digest construction;
- key activation, retirement, overlap, TTL, or skew;
- delegation signature or wire version;
- process-local replay admission;
- TCP framing, deadlines, listener lifecycle, channel selection, or loopback
  publication;
- ordinary persisted and process-local trigger behavior;
- existing manifests, features, direct dependencies, or `Cargo.lock`.

### Explicit non-claims

Slice 83 does not claim:

- executed PostgreSQL migration, transaction, or reconciliation evidence;
- crash-injection or network-partition evidence;
- durable records for denied or failed attempts;
- outbox publication, leasing, retries, retention, or external delivery;
- automatic recovery after fail-stop;
- coordinated clocks or distributed atomic activation;
- shared, durable, multi-replica, or restart-safe replay protection;
- HTTP, GraphQL, native RPC, MCP, CLI, signal, watcher, or polling triggers;
- secret zeroization, locked memory, TLS/mTLS, or non-loopback publication;
- successful compilation, tests, source-verifier execution, formatting,
  PostgreSQL execution, workflows, CI, or production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add isolated PostgreSQL execution for exact resume, audited CAS success,
   request/generation conflicts, concurrent CAS, transaction rollback, and
   commit-reconciliation fail-stop boundaries.
2. Add an explicit outbox dispatcher contract with bounded claim leases,
   idempotent delivery identity, retries, and retention.
3. Define an operator recovery ceremony for fail-stop, lost state, corruption,
   or externally advanced state.
4. Add clock-health and maximum-drift ownership before coordinated activation.
5. Replace process-local replay admission before claiming restart-safe or
   multi-replica replay prevention.

## Suggested verification — intentionally not run

- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-audit-outbox.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-postgres.mjs`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo test -p rustok-blog migrations`
- `cargo check -p rustok-server --features mod-blog --locked`

## Ownership retained

- Comments owns lifecycle validation, effective keyring selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns authorization, audited trigger composition, PostgreSQL
  state/outbox transaction logic, process-local audit, provider composition,
  listener lifecycle, concurrency, and shutdown.
- Blog owns both irreversible persistence migrations and remains
  transport-neutral for authenticated rendering and degraded presentation.
- A future dispatcher will own outbox claim, delivery, retry, and retention.
