# rustok-blog implementation plan — slice 72 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-71.md`. Slices 1–66 remain in
the original plan; slices 67–71 retain the typed remote core, TCP client,
trusted provider adapter, host-selected publication, and bounded listener
lifecycle.

## 2026-08-01 continuation audit

Slice 71 was rechecked against current `main` at
`86a4c3249ebca8f981b7f23deb67aac047420ff3`. The listener remains opt-in,
loopback-only while plaintext, bounded by frame/concurrency/idle/shutdown
limits, attached to the shared `StopHandle`, and separated from the
consumer-selected provider. The intervening Forum and Pricing commits do not
modify the Blog/Comments transport paths.

The current TCP payload carried no credential, so the listener could only start
when another host component supplied a concrete `CommentsTcpAuthorityResolver`.
This continuation adds the first concrete loopback service credential without
claiming transport encryption or remote-network safety.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 72 — authenticated Comments TCP bearer envelope

### Implemented source scope

- `crates/rustok-comments/src/tcp_auth.rs` owns the authentication envelope and
  concrete loopback bearer authority contract.
- `CommentsTcpRequestEnvelope` wraps one existing typed
  `CommentsThreadRequest` with:
  - `protocol_version`, currently `1`;
  - an optional typed credential;
  - the unchanged request and complete `PortContext`.
- Client and server keep the existing four-byte big-endian bounded JSON framing
  and one-request/one-reply connection scope.
- The server rejects unsupported envelope versions with
  `comments.tcp_server_unsupported_protocol` before authority or provider
  dispatch.
- `CommentsTcpBearerToken` accepts only 1..=4096 visible non-whitespace ASCII
  bytes. Empty, oversized, whitespace-containing, control-byte, and non-ASCII
  credentials fail configuration closed.
- The bearer value is retained privately for client envelope creation. Its
  `Debug` output, the wire credential `Debug`, resolver `Debug`, and transport
  `Debug` redact the secret.
- `crates/rustok-api/src/digest.rs` provides the already-dependency-backed shared
  SHA-256 helper and a fixed-work comparison over two 32-byte digests.
- Expected and candidate `Bearer <token>` values are hashed before comparison.
  Digest comparison has no early return or length-dependent loop. This source
  contract does not claim a separately audited compiler or hardware side-channel
  guarantee.
- No new direct dependency was added to `rustok-comments`; its manifest returned
  to the pre-slice dependency set, so this source change does not require a
  `Cargo.lock` package-entry mutation.
- Missing, malformed, incorrect, non-loopback, or non-canonical-tenant
  authentication inputs share the static error code and message:
  `comments.tcp_authentication_failed` / `Comments TCP service authentication
  failed`.
- `CommentsTcpBearerAuthorityResolver` authenticates the service credential,
  validates a canonical UUID tenant, and applies an explicit operation allowlist
  before returning trusted authority.
- Disallowed authenticated operations fail with
  `comments.tcp_operation_forbidden` before provider dispatch.
- `TcpJsonCommentsServerAdapter` passes the envelope credential only to the
  authority resolver. It never copies the credential into `PortContext`.
- After authentication, the existing tenant equality check remains mandatory and
  actor, claims, and roles are replaced with trusted values before all seven
  owner operations.
- `TcpJsonCommentsTransport` can construct authenticated version-1 envelopes and
  still retains an unauthenticated constructor for a future externally
  authenticated channel such as mTLS.
- Host `tcp` consumer publication now requires
  `RUSTOK_COMMENTS_TCP_BEARER_TOKEN` and uses the authenticated transport.
- An enabled listener keeps accepting an externally preconfigured
  `SharedCommentsTcpAuthorityResolver` as the highest-priority authority source.
- Without that override, the listener composes the concrete bearer resolver from:
  - `RUSTOK_COMMENTS_TCP_BEARER_TOKEN`;
  - `RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID`, which must be a valid UUID service
    actor without surrounding whitespace.
- The built-in service authority publishes exactly one platform permission claim,
  `comments:manage`, and one role, `admin`. These values are compatible with the
  existing `SecurityContext::try_from_port_context` contract and permit owner
  policy to remain authoritative.
- All seven Comments TCP operations are allowed by the built-in resolver; callers
  may construct a narrower resolver through `with_allowed_operations`.
- Existing loopback bind and endpoint restrictions remain unchanged because the
  bearer credential does not encrypt the transport.
- Historical slice-68 through slice-71 source guards are reconciled with the
  versioned authenticated envelope without changing their retained source-only
  evidence status.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-bearer-auth.json`.
- The standalone source verifier is
  `scripts/verify/verify-blog-comments-tcp-bearer-auth.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- TLS, mTLS, payload encryption, channel binding, or non-loopback safety;
- nonce, timestamp, challenge-response, request signing, or replay resistance;
- a separately audited compiler/hardware constant-time guarantee;
- bearer-token rotation, overlapping key sets, revocation, or secret-manager
  loading;
- per-tenant credentials or dynamic principal lookup;
- DNS/service discovery or multi-host publication;
- retry/backoff, circuit breaking, or write replay policy;
- startup connection probing, listener readiness, or provider health evidence;
- in-process/TCP parity execution;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add TLS/mTLS or an equivalently authenticated encrypted channel before any
   non-loopback endpoint or listener is allowed.
2. Prefer certificate identity or add bounded credential rotation and replay
   resistance if bearer authentication remains supported.
3. Add bounded retry/backoff only for retryable client failures. Writes must never
   be replayed without the existing idempotency key, and the original deadline
   must bound the complete attempt set.
4. Add in-process/TCP parity fixtures for all seven operations, protocol-version
   rejection, missing/wrong credentials, operation denial, tenant mismatch,
   exact provider errors, malformed and oversized frames, idle peers,
   concurrency rejection, disconnects, deadlines, and shutdown.
5. Add listener health/readiness only after retained runtime execution establishes
   bind, authentication, dispatch, and drain behavior.
6. Run and record retained Rust, JavaScript, TCP, PostgreSQL, browser, workflow,
   and CI targets before promoting evidence beyond source-only status.
7. Continue cached thread snapshot and comment-form fallback work after the
   authenticated remote path has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-bearer-auth.mjs`
- `node scripts/verify/verify-blog-comments-tcp-listener-lifecycle.mjs`
- `node scripts/verify/verify-blog-comments-tcp-server-adapter.mjs`
- `node scripts/verify/verify-blog-comments-tcp-transport.mjs`
- `cargo test -p rustok-api --lib digest::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_auth::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_server::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_transport::tests`
- `cargo test -p rustok-server --features mod-blog --lib comments_provider_runtime::tests`
- `cargo check -p rustok-server --features mod-blog --locked`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns typed request/reply envelopes, credential validation, concrete
  bearer authentication, owner dispatch, framing, and stable transport errors.
- `rustok-api` owns the narrow shared digest helper already backed by its existing
  SHA-256 dependency.
- Blog owns consumer policy, article rendering, degraded presentation, and
  transport-neutral `Arc<dyn CommentsThreadPort>` consumption.
- The server host owns secret and actor configuration, provider selection,
  endpoint and listener policy, authority override composition, concurrency, and
  shutdown.
- Authentication must fail closed and must not reveal whether a token, tenant, or
  peer check failed. Provider dispatch must occur only after trusted authority is
  established and applied.
- Plaintext bearer mode remains loopback-only. Bearer authentication alone must
  never be used to justify non-loopback publication.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
