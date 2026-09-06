# rustok-blog implementation plan — slice 80 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-79.md`. Slices 1–79 retain
the typed Comments remote boundary, bounded framing and listener lifecycle,
bearer-authenticated reads, signed user-write delegation, overlapping keyrings,
process-local whole-snapshot reload, time-aware activation and retirement, and
one process-local replay gate across generation and key lifecycle transitions.

## 2026-08-01 continuation audit

Slice 79 exposed low-level `replace_host_schedule(...)` and `reload_file()`
methods directly on the exported schedule handle. Those methods applied strict
schedule invariants, but authorization and audit ownership remained outside the
call boundary. A caller could invoke a valid mutation without first proving a
trusted principal or retaining a bounded attempt record.

Slice 80 places schedule mutation behind one host-injected authorizer and one
bounded process-local audit owner. It does not add an HTTP, GraphQL, native RPC,
MCP, signal, file-watch, or polling trigger. Host code decides how an already
authenticated control-plane operation reaches the programmatic trigger.

Tests, source verifiers, formatting, Cargo commands, TCP execution, workflows,
and CI remain intentionally unexecuted by request.

## Slice 80 — authorized schedule trigger and bounded audit

### Read-only exported schedule handle

The complete slice-79 schedule owner is preserved byte-for-byte as
`comments_provider_runtime_keyring_schedule_base.rs` and included through a
private historical module.

The exported `SharedCommentsTcpDelegationScheduleHandle` is now a read-only
wrapper. It retains:

- `from_host_schedule(...)`;
- `from_file(...)`;
- `current_status()`;
- `current_selection()`;
- `CommentsTcpDelegationKeyringProvider` behavior;
- redacted `Debug` behavior.

The raw mutation methods are `pub(super)` on the wrapper and are reachable only
from sibling server-owned trigger code. External callers can no longer mutate an
exported handle without traversing the authorized trigger boundary.

Initial environment-backed schedule composition remains supported. Such a
schedule is read-only after composition unless the host constructs and inserts a
trigger around a programmatic file-backed handle.

### Programmatic trigger composition

`SharedCommentsTcpDelegationScheduleTrigger` is constructed with:

1. one `SharedCommentsTcpDelegationScheduleHandle`;
2. one mandatory `SharedCommentsTcpDelegationScheduleTriggerAuthorizer`;
3. an audit capacity within `1..=1024` records.

The default suggested audit capacity constant is 256. No permissive default
authorizer exists.

A host inserts the trigger into `ModuleRuntimeExtensions`. The trigger guard
rejects a separately inserted standalone public schedule handle, extracts the
trigger-owned handle, and then delegates to the unchanged slice-79 schedule and
runtime-policy guards.

The trigger provides only two mutation operations:

- `reload_file(context)` for a file-backed handle;
- `replace_host_schedule(context, schedule, generation)` for a host-backed
  handle.

Source-category, generation, active-key, retained-key, overlap, propagation,
legacy compatibility, clock-floor, TTL, and skew validation remain owned by the
slice-79 schedule implementation.

### Trusted request context

Every trigger attempt requires a
`CommentsTcpDelegationScheduleTriggerContext` containing:

- a non-nil request UUID;
- a non-nil actor UUID;
- one already-classified `rustok_api::AuthPrincipalKind`.

The trigger does not accept a bearer token, session token, OAuth grant string,
client ID, role list, arbitrary claims, tenant supplied by a request body, or
caller-provided audit identity.

`DelegatedUser` is always ineligible for this host-global control-plane
operation and is rejected before the host authorizer is invoked. `DirectUser`
and `Service` principals still require an explicit authorizer approval. This
supports a human operator or dedicated automation principal without allowing a
delegated tenant OAuth user to reach the host schedule boundary.

### Host authorizer seam

`CommentsTcpDelegationScheduleTriggerAuthorizer` receives one bounded
`CommentsTcpDelegationScheduleTriggerAuthorizationRequest` with:

- request UUID;
- actor UUID;
- typed principal kind;
- closed operation enum;
- source category (`HostProvided` or `File`);
- current generation;
- optional requested generation.

It returns only:

- success;
- `Denied`;
- `Unavailable`.

No free-form authorizer reason is retained or exposed by the trigger. The host
may implement its policy using a direct-user control-plane rule, a dedicated
service capability, or another already-authenticated local authority. The
trigger itself remains independent of Axum, GraphQL, OAuth token parsing, and
RBAC storage.

The authorizer is called while the trigger operation mutex is held. An
authorizer implementation must therefore be non-reentrant and must not invoke
the same trigger recursively.

### Serialized mutation boundary

Each attempt is serialized through one process-local operation mutex. The
trigger then:

1. obtains an audit timestamp;
2. reads the current redacted schedule selection;
3. rejects an ineligible delegated principal or calls the mandatory authorizer;
4. acquires the audit mutex and reserves the next checked sequence number;
5. for an authorized attempt, invokes exactly one low-level mutation while the
   same audit guard remains held;
6. appends the final closed outcome before returning.

The audit mutex and sequence exhaustion are checked before mutation. The audit
ring is preallocated at construction. Once an authorized mutation starts under
that guard, final append has no fallible external I/O and no second lock
acquisition.

Therefore a successful trigger return always has a corresponding local
`ReplacementSucceeded` record. A rejected mutation has a
`ReplacementRejected` record. Authorization denial and unavailability are
recorded without invoking the mutation. A schedule preflight failure is recorded
as `PreflightRejected` when the audit owner remains available.

A panic or complete process failure is not converted into a durable transaction.
The audit is process-local and does not survive restart.

### Bounded audit record

`CommentsTcpDelegationScheduleTriggerAuditRecord` contains only:

- monotonically increasing process-local sequence;
- attempt timestamp;
- request UUID;
- actor UUID;
- typed principal kind;
- closed operation;
- closed outcome;
- optional source category;
- previous/current/requested generation metadata.

Closed outcomes are:

- `PreflightRejected`;
- `PrincipalIneligible`;
- `AuthorizationDenied`;
- `AuthorizationUnavailable`;
- `ReplacementRejected`;
- `ReplacementSucceeded`.

When the ring reaches capacity, the oldest record is removed before the newest
record is appended. Sequence numbers are not reused. Sequence overflow fails
closed before mutation.

The record does not contain:

- file path;
- active, retained, future, retired, or legacy key IDs;
- secret values;
- schedule JSON;
- credential or nonce data;
- raw mutation error text;
- bearer/session/OAuth tokens;
- arbitrary authorizer reason or metadata.

The trigger `Debug` output reports only the redacted handle, configured
authorizer presence, audit capacity, and record count. Actor and request UUIDs
are redacted from trigger `Debug`; they remain available only through the
explicit audit-record API.

### Audit availability policy

The trigger fails closed before mutation when:

- operation serialization state is poisoned;
- audit state is poisoned;
- the audit clock is unavailable;
- audit sequence is exhausted;
- schedule selection preflight fails;
- authorization is denied or unavailable.

There is no external sink in this slice. This intentionally avoids publishing a
new schedule and then discovering that an asynchronous database, broker, or
network audit write failed. Durable audit and transactional outbox composition
remain separate future ownership decisions.

### Preserved behavior

This slice does not change:

- schedule file schema version 2;
- activation, verification lead, retirement, overlap, TTL, and skew formulas;
- process-local monotonic schedule clock floor;
- source-category and generation replacement rules;
- retained key, secret, activation, retirement, and legacy policies;
- delegation scheme/version or keyed HMAC binding;
- request, actor, tenant, claims, role, operation, correlation, idempotency,
  digest, TTL, or clock verification;
- slice-78 replay continuity;
- service bearer reads and system moderation;
- external authority, channel, and provider precedence;
- loopback-only endpoint, bind, and peer policy;
- framing, deadlines, pre-request timeout, concurrency, listener lifecycle, and
  shutdown;
- manifest, feature, dependency, or lock-file state.

### Explicit non-claims

Slice 80 does not implement or claim:

- an HTTP, GraphQL, native RPC, MCP, CLI, or administrative route;
- automatic file watching or polling;
- signal handling;
- durable, database-backed, broker-backed, or external audit storage;
- transactional outbox publication;
- persisted generation or schedule digest;
- restart rollback prevention;
- cross-process audit sequence or deduplication;
- synchronized clocks or distributed atomic activation;
- cloud secret-manager, KMS, HSM, or sidecar SDK integration;
- shared, durable, multi-replica, or restart-safe replay protection;
- secret zeroization, locked memory, or file permission/ownership attestation;
- TLS/mTLS or non-loopback publication;
- successful compilation, tests, source-verifier execution, formatting, TCP
  execution, database execution, browser execution, workflow execution, CI, or
  production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Persist accepted generation and a redacted canonical schedule digest before
   claiming restart rollback prevention.
2. Bind durable audit/outbox publication to the same persisted generation
   transaction before claiming crash-safe audit completeness.
3. Add one concrete host transport only after its direct-user/service admission,
   CSRF/replay policy, rate limit, and response redaction are selected.
4. Define clock-health and maximum-drift ownership before coordinated activation
   across replicas.
5. Replace process-local replay admission with a bounded shared store before
   claiming multi-replica or restart-safe replay prevention.

## Suggested verification — intentionally not run

- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-trigger.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-key-schedule.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-keyring-reload.mjs`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo check -p rustok-server --features mod-blog --locked`

## Ownership retained

- Comments owns key lifecycle validation, effective keyring selection, signing,
  verification, request binding, trusted principal replacement, and process-local
  replay admission.
- The server host owns schedule source acquisition, mutation authorization,
  process-local audit, generation/replacement policy, runtime TTL selection,
  provider composition, listener lifecycle, concurrency, and shutdown.
- Blog remains transport-neutral and owns authenticated rendering and degraded
  presentation only.
