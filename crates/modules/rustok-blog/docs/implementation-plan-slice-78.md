# rustok-blog implementation plan — slice 78 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-77.md`. Slices 1–77 retain
the typed Comments remote boundary, bounded framing and listener lifecycle,
bearer-authenticated reads, signed user-write delegation, process-local replay
admission, channel injection, overlapping keyrings, and one validated immutable
host keyring snapshot shared by the built-in client and listener.

## 2026-08-01 continuation audit

Slice 77 deliberately published one immutable snapshot for the lifetime of the
composed transport and listener resolver. Replacing a file or constructing a new
host keyring after startup did not affect those already-published objects.
Rebuilding a resolver per generation would also reset its nonce cache and could
allow the same retained credential to be accepted again after rotation.

Slice 78 adds an explicit process-local reload owner. It does not add a watcher,
poller, signal handler, admin route, cloud SDK, or distributed coordination.
Host code decides when to invoke the reload handle. Tests, source verifiers,
formatting, Cargo commands, TCP execution, workflows, and CI remain intentionally
unexecuted by request.

## Slice 78 — atomic process-local delegation reload

### Opt-in composition

Reloadable composition is selected in either of two ways:

1. the host inserts a `SharedCommentsTcpDelegationKeyringReloadHandle` into
   `ModuleRuntimeExtensions`; or
2. `RUSTOK_COMMENTS_TCP_DELEGATION_RELOAD_ENABLED=true` is configured together
   with `RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE`.

When neither condition is present, the slice-77 static keyring path is called
unchanged. A static snapshot and a reload handle cannot be combined. The
reloadable path also rejects the historical single-secret environment profile.

The file-backed handle stores the initially selected path privately. Later file
reloads always read that same path; changing the environment variable does not
switch sources or paths inside the running process.

### Explicit host API

`SharedCommentsTcpDelegationKeyringReloadHandle` exposes:

- `from_host_keyring(...)` for a programmatic initial keyring;
- `from_file(...)` for a bounded file-backed initial keyring;
- `replace_host_keyring(...)` for programmatic replacement of a host-backed
  handle;
- `reload_file()` for replacement of a file-backed handle;
- `current_status()` and `current_selection()` for bounded metadata.

Source categories are immutable. A file-backed handle cannot accept a
programmatic replacement, and a host-backed handle cannot reload a file.

### Atomic replacement and generation policy

Each candidate is constructed as one complete immutable snapshot containing:

- the validated Comments-owned keyring;
- source category;
- positive generation;
- retained-key count;
- bounded revoked-key metadata count;
- legacy-unkeyed compatibility state.

The candidate is completely read, decoded, and validated before acquiring the
write lock. Replacement then:

1. acquires the process-local snapshot write lock;
2. rechecks that source category is unchanged;
3. rechecks that candidate generation is strictly greater than the generation
   currently active under the lock;
4. replaces the whole snapshot in one assignment;
5. increments a bounded metadata counter for successful reload attempts.

Equal or lower generations are rejected. Concurrent reloads are serialized by
the same write lock, so a candidate that was newer when prepared but stale by
the time it acquires the lock is rejected. Parse, validation, source, generation,
or lock failures leave the previous snapshot active and increment the rejected
attempt counter.

Generation monotonicity is process-local only. It is not persisted and therefore
does not prevent rollback across a complete process restart.

### Operation snapshot semantics

The Comments owner adds additive reloadable types without changing the existing
static signer, resolver, or transport:

- `CommentsTcpDelegationKeyringProvider` returns one validated keyring clone;
- `ReloadableCommentsTcpDelegationSigner` calls the provider once before issuing
  one user-write credential;
- `ReloadableCommentsTcpDelegatingAuthorityResolver` calls the provider once
  before verifying one delegated write;
- `ReloadableTcpJsonCommentsTransport` uses the reloadable signer for user-owned
  writes and the unchanged bearer for reads and system moderation.

An operation that has already selected a keyring completes against that immutable
snapshot. A later operation sees the newly published snapshot. No operation can
observe a key list, active key, or legacy policy assembled from two generations.

### Replay continuity across rotation

The reloadable resolver owns one process-local replay gate for its entire
lifetime. It does not replace that gate when the keyring generation changes.

The ordinary Comments resolver first completes signature, claim, request,
tenant, actor, operation, TTL, clock, and keyring validation against the one
selected snapshot. Only after that succeeds, the reloadable resolver decodes the
already-verified payload to recover the nonce and expiry. It hashes the nonce to
a fixed-size replay identity and admits it through one bounded mutex-protected
map until the signed expiry plus the configured clock skew.

Using the verified nonce rather than the full signed credential preserves
cross-key replay semantics: the same nonce remains rejected even if a retained
key permits a differently signed representation in another generation.

This preserves one-use process-local admission when a retained key appears in
both the old and new generations. It does not claim shared, durable,
multi-replica, or restart-safe replay prevention.

### Redacted metadata

`CommentsTcpDelegationKeyringReloadStatus` contains only:

- the existing redacted selection metadata;
- successful reload count;
- rejected reload count.

`CommentsTcpDelegationKeyringReloadOutcome` contains only previous generation
and current redacted selection metadata. The reload handle has a custom `Debug`
implementation that never exposes:

- file path;
- active, retained, or revoked key IDs;
- secret values;
- delegation nonce or serialized credentials;
- operating-system error details.

The file and replay DTOs intentionally do not implement `Debug`, and the reload
modules contain no tracing or printing sinks.

### Preserved behavior

This slice preserves:

- the static slice-77 host keyring path when reload is not selected;
- the historical single-secret profile outside reload mode;
- delegation scheme and wire version;
- keyed HMAC domain and key-ID binding;
- request digest, tenant, actor, claims, role, operation, correlation,
  idempotency, TTL, and clock-skew validation;
- service bearer reads and system moderation;
- external authority, channel, and provider override precedence;
- loopback-only endpoint, bind, and peer policy;
- frame limits, operation deadlines, pre-request timeout, concurrency, listener
  lifecycle, and shutdown grace;
- manifest, feature, dependency, and lock-file state.

### Explicit non-claims

Slice 78 does not implement or claim:

- automatic file watching or polling;
- signal handling or an administrative reload endpoint;
- cloud secret-manager, KMS, HSM, or sidecar SDK integration;
- persisted monotonic generation or restart rollback prevention;
- scheduled activation or retirement timestamps;
- propagation windows across processes or replicas;
- distributed atomic replacement;
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

1. Add activation and retirement timestamps with a fail-closed overlap window
   bounded by delegation TTL, clock skew, and deployment propagation policy.
2. Add an explicit host trigger integration only after ownership, authorization,
   audit, and failure reporting are selected; do not silently add a watcher.
3. Replace process-local replay admission with a bounded shared store before
   claiming multi-replica or restart-safe replay prevention.
4. Complete the concrete rustls/tokio-rustls adapter only with a consistent
   manifest and lock update, TLS 1.3, mutual certificates, server-name
   validation, ALPN, and separately bounded handshakes.
5. Retain loopback-only publication until protected-channel runtime evidence
   covers all seven operations, failures, concurrency, deadlines, rotation, and
   shutdown.

## Suggested verification — intentionally not run

- `node scripts/verify/verify-blog-comments-tcp-delegation-keyring-reload.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-keyring-host.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-key-rotation.mjs`
- `node scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`
- `cargo test -p rustok-comments --features tcp-transport tcp_delegation_reload`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo check -p rustok-server --features mod-blog --locked`

## Ownership retained

- Comments owns signing, verification, one-operation snapshot selection, request
  binding, trusted principal replacement, and process-local replay admission.
- The server host owns source acquisition, generation policy, atomic snapshot
  publication, explicit reload invocation, provider composition, listener
  lifecycle, concurrency, and shutdown.
- Blog remains transport-neutral and owns authenticated rendering and degraded
  presentation only.
