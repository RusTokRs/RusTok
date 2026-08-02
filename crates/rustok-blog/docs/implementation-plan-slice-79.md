# rustok-blog implementation plan — slice 79 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-78.md`. Slices 1–78 retain
the typed Comments remote boundary, bounded framing and listener lifecycle,
bearer-authenticated reads, signed user-write delegation, overlapping keyrings,
host-owned immutable snapshots, explicit process-local whole-snapshot reload,
and one replay gate that survives generation replacement.

## 2026-08-01 continuation audit

Slice 78 allowed a host to replace a complete keyring safely, but activation and
retirement remained manual. A host could still publish a new signing key too
late for verifier propagation or remove an old verification key before the last
old signer, delegation TTL, and clock-skew window had elapsed.

Slice 79 adds a time-aware lifecycle schedule with bounded overlap validation.
It does not add a background watcher, polling loop, signal handler, admin route,
durable generation store, distributed clock guarantee, or cross-replica
coordinator. Each new signing or authorization operation evaluates the current
wall-clock time through the existing reloadable keyring-provider seam.

Tests, source verifiers, formatting, Cargo commands, TCP execution, workflows,
and CI remain intentionally unexecuted by request.

## Slice 79 — scheduled delegation activation and retirement

### Separate opt-in profile

Scheduled composition is selected in either of two ways:

1. the host inserts a `SharedCommentsTcpDelegationScheduleHandle` into
   `ModuleRuntimeExtensions`; or
2. `RUSTOK_COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED=true` is configured with
   `RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE`.

When neither condition is present, the slice-78 ordinary reload path is called
unchanged, which in turn preserves the slice-77 static and historical
single-secret profiles.

Scheduled composition rejects coexistence with:

- `SharedCommentsTcpDelegationKeyringSnapshot`;
- `SharedCommentsTcpDelegationKeyringReloadHandle`;
- `RUSTOK_COMMENTS_TCP_DELEGATION_RELOAD_ENABLED=true`;
- `RUSTOK_COMMENTS_TCP_DELEGATION_SECRET`;
- a file path when a programmatic schedule handle is supplied.

Boolean schedule and ordinary-reload environment values are validated before
routing. Invalid UTF-8 or unknown boolean text fails startup composition.

### Version-2 file contract

Schedule mode reuses the bounded keyring file path but requires
`schema_version: 2`:

```json
{
  "schema_version": 2,
  "generation": 9,
  "propagation_budget_ms": 15000,
  "legacy_unkeyed_key_id": "comments-2026-07",
  "keys": [
    {
      "key_id": "comments-2026-07",
      "secret": "<32-or-more-visible-ASCII-bytes>",
      "activates_at_unix_ms": 1785560000000,
      "retires_at_unix_ms": 1785560092000
    },
    {
      "key_id": "comments-2026-08",
      "secret": "<32-or-more-visible-ASCII-bytes>",
      "activates_at_unix_ms": 1785560040000
    }
  ]
}
```

The existing 1..=65536-byte regular-file limit and second 65537-byte read cap
remain in force. Unknown JSON fields, operating-system details, paths, key IDs,
and secrets do not enter public diagnostics.

The file carries the propagation budget and key lifecycle timestamps. The
maximum delegation TTL comes from the existing
`RUSTOK_COMMENTS_TCP_DELEGATION_TTL_MS` host policy. The verifier clock skew uses
the existing Comments default, so the schedule and resolver use the same
source-owned bounds.

### Comments-owned schedule invariants

`CommentsTcpDelegationSchedule` contains 1..=8 scheduled keys. Construction
requires:

- every activation timestamp is greater than zero;
- key IDs are unique;
- activation timestamps are unique;
- retirement, when present, is later than the same key's activation;
- propagation budget is within 1..=300000 milliseconds;
- maximum TTL is within 1..=30000 milliseconds;
- clock skew is within 0..=30000 milliseconds;
- every non-terminal key has a retirement timestamp;
- the terminal key has no retirement timestamp;
- optional legacy-unkeyed verification references a scheduled key.

Keys are sorted by activation time inside the immutable schedule.

For every predecessor/successor pair, the predecessor retirement must satisfy:

`retirement >= successor activation + propagation budget + max TTL + clock skew`

Overflow fails construction. This window permits a lagging process to continue
issuing the predecessor key for the propagation budget and keeps verification
available for the complete maximum delegation lifetime plus clock skew.

### Operation-time effective keyring

At the beginning of each new operation:

- the signing key is the latest key whose activation is not later than `now`;
- a future key becomes a verification key at
  `activation - propagation budget`;
- a retiring key remains a verification key through its retirement timestamp;
- retired keys are omitted after retirement;
- legacy-unkeyed verification is enabled only while its selected key is in the
  effective verification set.

The complete effective `CommentsTcpDelegationKeyring` is assembled before the
existing signer or resolver is invoked. An operation therefore receives one
immutable active key and one immutable verification set. It cannot observe a
mixed schedule transition.

If the clock is unavailable, no key is active, or the effective keyring cannot
be constructed, the operation fails closed with
`comments.tcp_delegation_schedule_unavailable`.

No background task is required. Time activation is evaluated synchronously at
the existing signer/resolver operation boundary.

### Safe schedule replacement

`SharedCommentsTcpDelegationScheduleHandle` supports:

- `from_host_schedule(...)`;
- `from_file(..., max_ttl)`;
- `replace_host_schedule(...)`;
- `reload_file()`;
- `current_status()` and `current_selection()`.

A replacement candidate is fully parsed and schedule-validated before the write
lock. Under the write lock, replacement rechecks:

- source category is unchanged;
- generation is strictly greater;
- runtime TTL and clock-skew policy are unchanged;
- propagation budget has not decreased;
- the currently active signing key is unchanged at replacement time;
- every key not yet retired is retained;
- retained secret and activation timestamp are unchanged;
- retained retirement does not move earlier;
- each newly introduced key activates no earlier than
  `now + propagation budget`;
- legacy-unkeyed selection does not change before the prior legacy key retires.

The complete snapshot is replaced in one assignment. Equal/lower generations,
late key introduction, early active-key changes, shortened retirements, removed
retained keys, source changes, invalid files, or lock failures leave the active
snapshot unchanged.

The new-key lead-time rule applies to replacement. Initial process startup can
validate only its local schedule; it cannot prove that every replica received
the schedule before a historical activation time. No distributed rollout claim
is made.

### Replay continuity

The schedule handle implements the same
`CommentsTcpDelegationKeyringProvider` used by slice 78. The built-in client uses
`ReloadableCommentsTcpDelegationSigner`, and the listener uses
`ReloadableCommentsTcpDelegatingAuthorityResolver`.

The listener resolver therefore retains one bounded process-local nonce replay
gate across:

- generation replacement;
- pre-activation verification overlap;
- signing-key activation;
- predecessor retirement.

Service bearer reads and system moderation remain outside user delegation.
Shared, durable, multi-replica, and restart-safe replay protection remain open.

### Redacted status

Schedule status exposes only:

- source category;
- generation;
- scheduled-key count;
- current verification-key count;
- propagation budget;
- maximum TTL;
- clock skew;
- whether legacy-unkeyed verification is currently effective;
- successful and rejected replacement counters.

It does not expose paths, active/retained/retired key IDs, secrets, credentials,
nonces, or operating-system error details. Scheduled-key and schedule `Debug`
implementations redact identifiers and secrets.

### Preserved behavior

This slice does not change:

- delegation credential scheme or wire version;
- keyed HMAC domain or key-ID binding;
- request digest, tenant, actor, claims, role, operation, correlation,
  idempotency, TTL, or clock validation;
- service bearer reads and system moderation;
- external authority, channel, and provider precedence;
- loopback-only endpoint, bind, and peer policy;
- framing, operation deadlines, pre-request timeout, concurrency, listener
  lifecycle, and shutdown;
- static or ordinary reload source files;
- manifest, feature, dependency, or lock-file state.

### Explicit non-claims

Slice 79 does not implement or claim:

- automatic file watching or polling;
- signal handling or an administrative reload endpoint;
- authenticated trigger authorization or an audit event stream;
- persisted monotonic generation or restart rollback prevention;
- synchronized clocks across processes or replicas;
- distributed schedule publication or atomic activation;
- cloud secret-manager, KMS, HSM, or sidecar SDK integration;
- shared or durable replay storage;
- secret zeroization or locked memory;
- file permission or ownership attestation;
- TLS/mTLS or non-loopback publication;
- retry/backoff, readiness, health, or runtime evidence;
- successful compilation, tests, source-verifier execution, formatting, TCP
  execution, database execution, browser execution, workflow execution, CI, or
  production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add an explicitly authorized host trigger with bounded audit records for file
   and programmatic schedule replacement; do not add an unauthenticated route.
2. Persist accepted generation and schedule digest before claiming restart
   rollback prevention.
3. Define clock-health and maximum-drift ownership before claiming coordinated
   activation across replicas.
4. Replace process-local replay admission with a bounded shared store before
   claiming multi-replica or restart-safe replay prevention.
5. Complete the concrete rustls/tokio-rustls adapter only with a consistent
   manifest and lock update, mutual TLS 1.3, server-name validation, ALPN, and
   separately bounded handshakes.

## Suggested verification — intentionally not run

- `node scripts/verify/verify-blog-comments-tcp-delegation-key-schedule.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-keyring-reload.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-keyring-host.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-key-rotation.mjs`
- `cargo test -p rustok-comments --features tcp-transport tcp_delegation_schedule`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo check -p rustok-server --features mod-blog --locked`

## Ownership retained

- Comments owns schedule validation, operation-time effective keyring selection,
  signing, verification, request binding, trusted principal replacement, and
  process-local replay admission.
- The server host owns source acquisition, generation and replacement policy,
  runtime TTL selection, explicit trigger invocation, provider composition,
  listener lifecycle, concurrency, and shutdown.
- Blog remains transport-neutral and owns authenticated rendering and degraded
  presentation only.
