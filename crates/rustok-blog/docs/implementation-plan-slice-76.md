# rustok-blog implementation plan — slice 76 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-75.md`. Slices 1–66 remain in
the original plan; slices 67–75 retain the typed Comments remote core, bounded
framing and listener lifecycle, bearer-authenticated reads, signed user
write delegation, process-local replay admission, generic channel interfaces,
and host-selected channel wiring.

## 2026-08-01 continuation audit

The planned concrete rustls/tokio-rustls adapter was implemented on an isolated
technical branch, but adding its direct dependencies also requires a precise
`Cargo.lock` graph update. The available repository connector cannot patch a
small range in the large lock file, and this continuation is not permitted to
run Cargo to regenerate it. The incomplete dependency graph is therefore not
merged or promoted.

The next independent security result from the plan is delegation key rotation.
The existing protocol used one HMAC secret with no key identifier, so changing
that secret would invalidate all in-flight delegations and could not support an
overlap window. This bounded slice adds a lock-compatible keyring core without
changing dependencies, the host environment profile, transport routing, or the
process-local replay model.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 76 — delegation key IDs and overlapping rotation core

### Implemented source scope

- `crates/rustok-comments/src/tcp_delegation.rs` now owns a bounded keyring.
- `CommentsTcpDelegationKeyId` accepts 1..=64 ASCII letters, digits, dots,
  underscores, or hyphens.
- `CommentsTcpDelegationKeyring` requires:
  - 1..=8 unique keys;
  - exactly one configured active signing key;
  - an active key ID that exists in the verification set.
- Key secrets retain the existing 32..=4096 visible non-whitespace ASCII
  contract and redacted `Debug` behavior.
- Keyring `Debug` exposes only key count and whether legacy-unkeyed verification
  is enabled. Active IDs and all secret values are redacted.
- `CommentsTcpDelegationSigner::with_keyring` and
  `with_keyring_and_ttl` issue new delegations with the active key ID.
- New signatures bind, in order:
  - the existing delegation signature domain;
  - the bounded key ID;
  - a separator byte;
  - the complete serialized claims payload.
- The verifier parses only the small signed envelope first, validates the key ID,
  selects one verification key, verifies the HMAC, and only then parses claims.
- Unknown, malformed, or removed key IDs fail with the existing generic
  `comments.tcp_delegation_invalid` code and message.
- No public error distinguishes an unknown key from a bad signature, invalid
  claims, altered request, or expired delegation.
- Overlap rotation is supported by keeping old and new secrets in one keyring
  while switching only the active signing key.
- Revocation is represented by removing a verification key from the next
  keyring snapshot. Tokens signed by that key then fail closed.
- `with_legacy_unkeyed_key_id` explicitly assigns one verification key for
  tokens created before key IDs existed.
- The retained single-secret constructors create a one-key `legacy` keyring,
  issue new keyed tokens, and continue accepting old unkeyed tokens. Existing
  host environment configuration therefore remains source-compatible.
- The legacy unkeyed HMAC format is accepted only when a keyring explicitly
  retains a legacy key. A newly constructed multi-key ring has no implicit
  legacy fallback.
- Delegation version, credential scheme, request digest, user/context bindings,
  TTL bounds, clock-skew bounds, operation routing, principal replacement, and
  process-local nonce admission remain unchanged.
- Replay nonces remain global within the listener process rather than scoped by
  key ID, preventing the same nonce from being admitted once per rotating key.
- `crates/rustok-comments/src/lib.rs` exports the key ID, keyring, and bounds.
- No manifest, direct dependency, feature, or `Cargo.lock` change is required.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-key-rotation.json`.
- The standalone source verifier is
  `scripts/verify/verify-blog-comments-tcp-delegation-key-rotation.mjs`.

### Recommended rotation sequence

1. Publish a verification keyring containing the old and new keys while the old
   key remains active.
2. Publish the same verification set with the new key active for signing.
3. Wait at least the maximum delegation TTL plus permitted clock skew and
   deployment propagation time.
4. Remove the old verification key.
5. Disable legacy-unkeyed verification after all pre-key-ID deployments and
   tokens are outside the accepted window.

The keyring is immutable after construction. Hosts should publish a new signer
and resolver snapshot rather than mutate a live key map.

### Explicit non-claims

This slice does not implement or claim:

- environment JSON or file parsing for multiple delegation keys;
- secret-manager loading, live reload, or atomic host snapshot replacement;
- durable key metadata, audit history, scheduled activation, or scheduled
  revocation;
- asymmetric signing, hardware-backed keys, KMS HMAC, or independently audited
  side-channel guarantees;
- cluster-wide, multi-process, multi-replica, durable, or restart-safe replay
  prevention;
- TLS/mTLS, certificate loading, or non-loopback publication;
- retry/backoff, readiness, health, or runtime evidence;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add fail-closed host keyring composition from a bounded secret-manager/file
   source, with one immutable signer/resolver snapshot and no secret logging.
2. Add explicit activation/revocation metadata and overlapping reload semantics.
3. Replace process-local nonce admission with a bounded shared replay store
   before claiming multi-replica or restart-safe replay prevention.
4. Complete the concrete rustls/tokio-rustls adapter only with a consistent
   manifest and lock-file update, TLS 1.3, mutual certificates, server-name
   validation, ALPN, and separately bounded handshakes.
5. Retain loopback-only publication until protected-channel runtime evidence
   covers all seven operations, failures, concurrency, deadlines, and shutdown.
6. Add retry/backoff and cached comment fallback only after protected remote-path
   runtime evidence exists.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-delegation-key-rotation.mjs`
- `node scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`
- `node scripts/verify/verify-blog-comments-tcp-bearer-auth.mjs`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_delegation::tests`
- `cargo check -p rustok-server --features mod-blog --locked`

## Boundaries retained

- Comments owns signed delegation format, key selection, request/context binding,
  principal replacement, replay admission, and stable transport errors.
- The server host still owns secret acquisition, keyring publication, authority
  composition, provider selection, channel selection, listener policy,
  concurrency, and shutdown.
- Blog owns authenticated user/moderation contexts, article rendering, and
  degraded presentation through the transport-neutral Comments port.
- Key IDs are routing metadata, not authorization principals and not secrets.
- Removing a key affects signature verification only; bearer/delegation policy,
  tenant equality, and owner authorization remain mandatory.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
