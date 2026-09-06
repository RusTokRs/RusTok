# rustok-blog implementation plan — slice 77 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-76.md`. Slices 1–66 remain in
the original plan; slices 67–76 retain the typed Comments remote core, bounded
framing and listener lifecycle, bearer-authenticated reads, signed user write
delegation, process-local replay admission, generic channel interfaces,
host-selected channel wiring, and overlapping delegation keyrings.

## 2026-08-01 continuation audit

Slice 76 added a bounded immutable keyring but deliberately left key acquisition
to the host. The server still knew only the legacy single-secret environment
variable, so a multi-key ring could be constructed programmatically by Rust code
but could not be selected through a bounded deployment artifact. Client signer
and listener resolver also needed an explicit composition point proving that both
consume the same immutable keyring snapshot.

This slice adds that host composition without adding dependencies, changing the
Comments wire format, relaxing loopback policy, or claiming live reload. No
compile, Rust or JavaScript test, TCP exchange, database, browser, workflow, CI,
production, or runtime result is promoted. Execution remains maintainer-owned.

## Slice 77 — host-owned delegation keyring snapshot

### Runtime module boundary

- The historical implementation is retained byte-for-byte at
  `apps/server/src/services/comments_provider_runtime_base.rs`.
- `apps/server/src/services/comments_provider_runtime.rs` is now a small facade
  that re-exports the historical public API and intercepts only:
  - provider registration;
  - optional listener startup.
- The new implementation lives in
  `apps/server/src/services/comments_provider_runtime_keyring.rs`.
- Legacy configurations with no multi-key snapshot delegate to the historical
  implementation unchanged.

### Supported snapshot sources

The host resolves one source with strict precedence and ambiguity rejection:

1. a programmatic `SharedCommentsTcpDelegationKeyringSnapshot` already inserted
   into `ModuleRuntimeExtensions`;
2. the file named by `RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE`;
3. the historical `RUSTOK_COMMENTS_TCP_DELEGATION_SECRET` path, handled by the
   historical implementation when neither multi-key source exists.

A programmatic snapshot is the integration seam for a host secret-manager,
sidecar, or deployment-specific adapter. This slice does not add a cloud secret
manager SDK. The built-in file source can consume an ordinary host file or a
file mounted by an external secret manager.

The host rejects these ambiguous combinations before publishing any new runtime
extension:

- programmatic snapshot plus keyring file;
- programmatic snapshot plus legacy secret;
- keyring file plus legacy secret.

### File contract

`RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE` references one regular file:

- file content size is hard-bounded to 1..=65536 bytes;
- reads use a second 65537-byte cap so metadata races cannot bypass the limit;
- the path and operating-system error details are not copied into public errors;
- the document must be valid UTF-8 JSON accepted by `serde_json`;
- unknown fields are rejected;
- `schema_version` must equal `1`;
- `generation` must be greater than zero;
- `keys` is validated by the Comments-owned 1..=8 immutable keyring contract;
- every retained key ID and secret is validated by the Comments owner;
- `revoked_key_ids` contains at most eight valid unique key IDs;
- retained and revoked key IDs must be disjoint;
- `active_key_id` must resolve to a retained key;
- optional `legacy_unkeyed_key_id` must resolve to a retained key.

Example deployment shape, with non-production placeholders:

```json
{
  "schema_version": 1,
  "generation": 7,
  "active_key_id": "comments-2026-08",
  "legacy_unkeyed_key_id": "comments-2026-07",
  "revoked_key_ids": ["comments-2026-06"],
  "keys": [
    {"key_id": "comments-2026-07", "secret": "<32-or-more-visible-ASCII-bytes>"},
    {"key_id": "comments-2026-08", "secret": "<32-or-more-visible-ASCII-bytes>"}
  ]
}
```

`revoked_key_ids` is bounded source metadata. Effective cryptographic revocation
still comes from omitting the old key from `keys`; it is not a durable denylist.
`generation` identifies the host snapshot but this slice does not persist or
enforce monotonic generations across process restarts.

### One immutable snapshot

- The complete file is read and validated before any extension is inserted.
- A keyring, client port, provider selection, and redacted runtime selection are
  prepared locally.
- Publication occurs only after all fallible preparation succeeds.
- The client signer receives a clone of the immutable keyring snapshot during
  provider registration.
- The snapshot itself is inserted into `ModuleRuntimeExtensions`.
- Immediately before listener startup, the facade resolves the same snapshot
  from `ServerRuntimeContext` and composes the built-in listener authority.
- A runtime-context or module-extension `SharedCommentsTcpAuthorityResolver`
  continues to take precedence; the snapshot does not overwrite an external
  authority resolver.
- The historical listener lifecycle, bind, concurrency, frame, timeout, and
  shutdown owner remains unchanged.

A configured snapshot must be consumed by at least one built-in side:

- a non-preconfigured `tcp` client provider; or
- an enabled Comments TCP listener.

An otherwise unused snapshot fails startup composition rather than silently
claiming rotation coverage.

### Redacted runtime metadata

`CommentsTcpDelegationKeyringRuntimeSelection` exposes only:

- source category: `HostProvided` or `File`;
- positive generation;
- retained-key count;
- revoked-key metadata count;
- whether legacy-unkeyed verification is enabled.

It contains no path, active key ID, retained/revoked key IDs, or secret value.
`SharedCommentsTcpDelegationKeyringSnapshot` has a custom redacted `Debug`
implementation. Existing Comments keyring and secret redaction remains in force.

### Preserved security and behavior

This slice does not change:

- delegation credential scheme or version;
- key-ID HMAC binding;
- signature-before-claims verification ordering;
- tenant, actor, claims, role, operation, correlation, idempotency, request
  digest, TTL, and clock-skew bindings;
- process-local replay admission shared across all retained keys;
- service bearer reads and system moderation;
- external authority/channel/provider precedence;
- provider transport selection profiles;
- plaintext loopback enforcement;
- listener bind and peer loopback enforcement;
- pre-request timeout, concurrency, framing, and shutdown behavior;
- the historical single-secret environment contract when no keyring snapshot is
  configured.

### Explicit non-claims

This slice does not implement or claim:

- cloud secret-manager SDK integration;
- secret zeroization or locked memory;
- file permission/ownership attestation;
- live file watching, polling, signal reload, or hot replacement;
- monotonic generation persistence or rollback prevention;
- scheduled activation, scheduled revocation, or durable key audit history;
- distributed atomic rotation across processes or replicas;
- shared, durable, multi-replica, or restart-safe replay prevention;
- asymmetric, KMS, HSM, or hardware-backed signing;
- TLS/mTLS or non-loopback publication;
- retry/backoff, readiness, health, or runtime evidence;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add explicit reload ownership with atomic whole-snapshot replacement and
   monotonic generation rejection inside one process.
2. Add activation and retirement timestamps with a fail-closed overlap window
   bounded by delegation TTL, clock skew, and deployment propagation policy.
3. Replace process-local nonce admission with a bounded shared replay store
   before claiming multi-replica or restart-safe replay prevention.
4. Complete the concrete rustls/tokio-rustls adapter only with a consistent
   manifest and lock-file update, TLS 1.3, mutual certificates, server-name
   validation, ALPN, and separately bounded handshakes.
5. Retain loopback-only publication until protected-channel runtime evidence
   covers all seven operations, failures, concurrency, deadlines, and shutdown.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-delegation-keyring-host.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-key-rotation.mjs`
- `node scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo check -p rustok-server --features mod-blog --locked`

## Boundaries retained

- Comments owns key IDs, keyring validity, signing, verification, request/context
  binding, principal replacement, replay admission, and stable transport errors.
- The server host owns secret acquisition, file/source limits, snapshot metadata,
  provider and authority composition, channel selection, listener lifecycle,
  concurrency, and shutdown.
- Blog owns authenticated user/moderation contexts, article rendering, and
  degraded presentation through the transport-neutral Comments port.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
