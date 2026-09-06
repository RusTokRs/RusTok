# rustok-blog implementation plan — slice 73 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-72.md`. Slices 1–66 remain in
the original plan; slices 67–72 retain the typed remote core, bounded TCP client
and server adapters, host-selected publication, listener lifecycle, versioned
credential envelope, and loopback bearer-authenticated read profile.

## 2026-08-01 continuation audit

Slice 72 was rechecked against current `main` at
`6cb1dde6071708a2d72328e7c01d2dc74a9c4931`. The intervening Region, Tax, and
other ecommerce diagnostic commits do not modify Blog, Comments, the shared
digest helper, or the TCP host runtime.

The bearer-only profile remains deliberately read-only. Blog create, update, and
delete calls already construct user-owned `PortContext` values with a canonical
user UUID, permission claims, one role, a deadline, correlation identity, and a
non-empty idempotency key. Blog moderation is different: Blog first enforces its
management policy, then intentionally calls the Comments owner with a system
actor. This continuation preserves that distinction instead of pretending every
write has the same authority model.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 73 — signed user delegation for Comments TCP writes

### Implemented source scope

- `crates/rustok-api/src/digest.rs` adds a narrow HMAC-SHA256 helper backed by
  the crate's existing SHA-256 dependency.
- The helper follows RFC 2104 key normalization, accepts ordered message chunks,
  and adds no dependency or `Cargo.lock` package-entry change.
- `crates/rustok-comments/src/tcp_delegation.rs` owns the signed delegation
  contract.
- `CommentsTcpDelegationSecret` accepts 32..=4096 visible non-whitespace ASCII
  bytes and redacts its value from `Debug`.
- `CommentsTcpDelegationSigner` issues version-1 credentials using the distinct
  `delegated_hmac_sha256` scheme.
- The signature uses HMAC-SHA256 over a fixed protocol domain separator and the
  exact serialized delegation claims payload.
- Signed claims bind:
  - tenant UUID;
  - end-user UUID;
  - complete permission-claim vector;
  - exactly one role;
  - typed operation identity;
  - correlation id;
  - idempotency key;
  - issued-at and expiry timestamps;
  - a canonical UUID nonce;
  - SHA-256 of the complete serialized `CommentsThreadRequest`, including its
    operation payload and complete `PortContext`.
- Signing requires existing write-port semantics before any network connection:
  a deadline and non-empty idempotency key remain mandatory.
- Delegations default to a 5-second lifetime and may not exceed 30 seconds.
- Delegation payload and final credential token have explicit byte bounds.
- `CommentsTcpAuthorityResolver` now receives the complete immutable typed
  request before trusted principal replacement and provider dispatch.
- `CommentsTcpOperation` exposes stable operation labels, write classification,
  and request-derived operation identity.
- The server rejects operation/request mismatches before authority is returned.
- `CommentsTcpDelegatingAuthorityResolver` composes two authority paths:
  - service bearer for the three existing reads;
  - service bearer for `SetCommentStatus` only when the incoming Blog context is
    the host-owned system actor;
  - signed user delegation for `CreateComment`, `UpdateComment`, and
    `DeleteComment`.
- Delegated writes require the signed tenant, actor, claims, role, operation,
  correlation id, idempotency key, request digest, timestamps, and nonce to match
  the exact request presented to the server.
- Successful delegated writes replace the untrusted principal with the verified
  user actor, claims, and role before existing Comments owner policy executes.
- Existing owner authorization remains decisive:
  - create still requires a user id;
  - update/delete still enforce permission and ownership scope;
  - moderation still requires Comments moderate/manage authority.
- The client transport signs user-owned writes only when a delegation signer is
  configured. Reads and current Blog system moderation use the service bearer.
- Signature creation, claims serialization, request-envelope encoding, connect,
  write, read, reply decode, and provider response all remain bounded by the
  original `PortContext` deadline.
- A nonce is accepted once by a process-local replay cache shared by all cloned
  listener adapters.
- Expired nonce entries are pruned before admission.
- Replay capacity defaults to 4096 and is hard-bounded to 1..=65536. A full or
  unavailable cache fails closed before provider dispatch.
- Duplicate nonce use in the same listener process returns
  `comments.tcp_delegation_replayed`.
- Host configuration is opt-in:
  - `RUSTOK_COMMENTS_TCP_DELEGATION_SECRET` enables signing and verification;
  - `RUSTOK_COMMENTS_TCP_DELEGATION_TTL_MS` selects the bounded lifetime;
  - `RUSTOK_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY` selects the bounded
    process-local cache capacity.
- Without the delegation secret, the client and listener retain the slice-72
  bearer-only read profile and owner writes continue to fail closed.
- An externally supplied `SharedCommentsTcpAuthorityResolver` still has priority
  over all built-in resolver composition.
- Plaintext endpoint, listener, and peer restrictions remain loopback-only.
- No secret, signature, signed token, or delegation payload is emitted through
  transport, resolver, or secret `Debug` output.
- The slice-72 bearer guard is reconciled so the original three-read default
  remains protected independently of the new delegation resolver.
- Slice 73 uses one comprehensive source guard for HMAC, credential routing,
  server authorization ordering, host configuration, Blog context construction,
  Comments owner policy, and process-local replay bounds.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-user-delegation.json`.
- The standalone source verifier is
  `scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- TLS, mTLS, payload encryption, channel binding, or non-loopback safety;
- cluster-wide, multi-process, multi-replica, or durable replay prevention;
- replay prevention across listener restarts;
- key identifiers, overlapping key sets, rotation, revocation, or secret-manager
  loading;
- asymmetric signatures or certificate-derived delegation;
- a separately audited compiler/hardware constant-time guarantee;
- automatic authorization minting from raw HTTP headers at this transport layer;
- retry/backoff, circuit breaking, or write replay policy;
- startup connection probing, listener readiness, or provider health evidence;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Replace plaintext loopback publication with TLS/mTLS or an equivalently
   authenticated encrypted local channel before permitting any non-loopback
   endpoint or listener.
2. Replace the process-local nonce cache with a bounded shared replay store before
   claiming multi-replica or restart-safe replay prevention.
3. Add key identifiers, bounded overlapping-key rotation, revocation policy, and
   secret-manager loading for bearer and delegation secrets.
4. Add bounded retry/backoff only for retryable failures. The original deadline
   must bound all attempts, and writes may be replayed only with the existing
   idempotency key and a newly issued delegation/nonce per attempt.
5. Add in-process/TCP parity fixtures for all seven operations and both authority
   paths. Cover altered bodies, actors, claims, roles, operations, correlation
   ids, idempotency keys, expiry, future issue times, nonce replay, cache pressure,
   protocol-version rejection, malformed and oversized frames, disconnects,
   deadlines, concurrency rejection, and shutdown.
6. Add retained PostgreSQL evidence for create/update/delete ownership behavior and
   service moderation only after the maintainer executes the corresponding
   targets.
7. Add listener readiness and health only after retained runtime evidence confirms
   bind, read bearer authentication, delegated writes, moderation, replay
   rejection, dispatch, and drain behavior.
8. Continue cached thread snapshot and comment-form fallback work after the
   authenticated remote path has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`
- `node scripts/verify/verify-blog-comments-tcp-bearer-auth.mjs`
- `cargo test -p rustok-api --lib digest::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_delegation::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_auth::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_server::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_transport::tests`
- `cargo test -p rustok-server --features mod-blog --lib comments_provider_runtime::tests`
- `cargo check -p rustok-server --features mod-blog --locked`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns the typed request/reply and credential envelopes, delegation
  claims, signing/verifying contract, request binding, owner dispatch, stable
  transport errors, and process-local replay admission.
- `rustok-api` owns the narrow shared SHA-256/HMAC helper already backed by its
  existing dependency.
- Blog owns authenticated user and moderation policy, article rendering,
  degraded presentation, and transport-neutral `Arc<dyn CommentsThreadPort>`
  consumption.
- The server host owns secret/TTL/cache configuration, endpoint and listener
  policy, resolver override composition, provider selection, concurrency, and
  shutdown.
- Signed claims are not self-authorizing. Existing Comments owner policy must run
  after verified principal replacement and remains authoritative.
- A delegation is valid only for the exact typed request and one nonce admission
  in the current listener process.
- Plaintext credential modes remain loopback-only and must never justify
  non-loopback publication.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
