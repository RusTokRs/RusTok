# rustok-blog implementation plan — slice 75 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-74.md`. Slices 1–66 remain in
the original plan; slices 67–74 retain the typed Comments remote core, bounded
framing and listener lifecycle, bearer-authenticated reads, signed user
delegation, process-local replay admission, and transport-owned client/server
channel interfaces.

## 2026-08-01 continuation audit

Slice 74 was rechecked against current `main` at
`60ddbaeb44fdabe9c1dfbfd55fab8bb6a1f32814`. The channel interfaces were present,
but the built-in server runtime still constructed the plaintext client transport
and invoked the plaintext listener adapter directly. A concrete encrypted
connector could therefore only be used by replacing the complete host runtime.

The next secure result is split at the host boundary. This slice makes client and
server channel implementations first-class runtime extensions and preserves the
plaintext loopback profile as the only built-in default. A later concrete rustls
adapter can now be inserted without changing Blog consumers, Comments framing,
listener concurrency, application credentials, authority replacement, or owner
policy.

No compile, Rust or JavaScript test, TCP exchange, TLS handshake, database,
browser, workflow, CI, production, or runtime result is promoted by this
continuation. Execution remains maintainer-owned.

## Slice 75 — host-selected protected Comments channel wiring

### Implemented source scope

- `apps/server/src/services/comments_provider_runtime.rs` now imports the
  transport-owned channel contracts from `rustok-comments`.
- `SharedCommentsTcpClientChannelConnector` is a typed
  `ModuleRuntimeExtensions` wrapper for a host-selected client connector.
- `SharedCommentsTcpServerChannelAcceptor` is a typed runtime wrapper for a
  host-selected accepted-stream channel implementation.
- Client connector selection occurs before the remote `CommentsThreadPort` is
  published:
  - a pre-inserted `SharedCommentsTcpClientChannelConnector` is used when
    present;
  - otherwise the host constructs `PlaintextLoopbackCommentsTcpChannel`.
- The selected connector is passed to
  `TcpJsonCommentsTransport::with_channel_connector_and_bearer_token` or
  `TcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation`.
- Bearer reads, system moderation, and signed user create/update/delete routing
  remain unchanged above the selected byte channel.
- `CommentsProviderProfile::TcpProtectedLoopback` distinguishes a connector that
  reports `AuthenticatedEncrypted` from the retained `TcpLoopback` plaintext
  profile.
- The profile name is descriptive source metadata only. It is not TLS evidence
  and does not mint Comments authority.
- Both plaintext and host-provided protected client connectors remain restricted
  to loopback endpoints in this slice.
- A non-loopback plaintext endpoint fails closed with a plaintext-specific
  configuration error.
- A non-loopback connector classified as authenticated and encrypted also fails
  closed until retained protected-channel runtime evidence exists.
- Server acceptor precedence is:
  1. `SharedCommentsTcpServerChannelAcceptor` in `ServerRuntimeContext`;
  2. the same wrapper in `ModuleRuntimeExtensions`;
  3. `PlaintextLoopbackCommentsTcpChannel`.
- The selected acceptor is captured once before the listener task is spawned and
  shared across connection tasks through `Arc`.
- Every accepted stream is passed to
  `handle_connection_with_acceptor_and_pre_request_timeout`.
- A host-provided acceptor must finish and bound its own handshake before it
  returns a byte channel. The existing pre-request timeout still starts after
  channel establishment and bounds the first complete typed frame.
- Listener bind addresses and accepted peer addresses remain loopback-only even
  when a protected acceptor is supplied.
- Listener startup diagnostics record only the closed channel-protection enum;
  no certificate, key, secret, signature, token, or channel implementation debug
  payload is logged.
- Existing listener semaphore, `JoinSet`, shared `StopHandle`, shutdown grace,
  drain/abort behavior, frame bounds, authority resolution, tenant equality,
  principal replacement, and provider dispatch remain unchanged.
- External authority override precedence remains independent of channel
  selection. An authenticated encrypted channel does not replace bearer or
  delegation authorization.
- The retained slice-70 host-provider and slice-71 listener guards are
  reconciled with the new runtime channel selection.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-host-channel-selection.json`.
- The standalone source verifier is
  `scripts/verify/verify-blog-comments-tcp-host-channel-selection.mjs`.
- No dependency or feature declaration is added, and `Cargo.lock` is unchanged.

### Explicit non-claims

This slice does not implement or claim:

- a rustls, tokio-rustls, native-tls, OpenSSL, or other concrete TLS adapter;
- TLS or mTLS negotiation;
- certificate, private-key, trust-root, DNS name, SNI, ALPN, expiry, or
  revocation validation;
- that `AuthenticatedEncrypted` is independently verified by the runtime;
- encrypted bytes in the built-in profile;
- non-loopback endpoint, bind, or peer enablement;
- channel-derived service or end-user authority;
- shared, durable, restart-safe, or multi-replica replay prevention;
- certificate/key rotation or secret-manager integration;
- retry/backoff, circuit breaking, readiness, or health evidence;
- successful compile, Rust test, JavaScript verifier, TCP/TLS runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add concrete rustls/tokio-rustls client and server channel implementations
   behind an explicit feature with a lock-consistent dependency update.
2. Require TLS 1.3, host-owned trust roots, server-name validation, client and
   server certificate chains, PKCS#8 private keys, and a separately bounded
   handshake.
3. Add fail-closed certificate and key loading without logging file contents or
   unbounded parser errors.
4. Keep bearer/delegation authorization and tenant equality above mTLS unless a
   separately designed certificate-to-service-authority mapping is introduced.
5. Retain loopback-only publication until mTLS handshake, all seven application
   operations, failure cases, concurrency, deadline, and shutdown behavior have
   retained runtime evidence.
6. Add certificate rotation, overlapping trust roots, revocation policy, and
   secret-manager loading.
7. Replace process-local delegation replay admission with a bounded shared store
   before claiming multi-replica or restart-safe replay prevention.
8. Add retry/backoff and cached comment fallback only after protected remote-path
   runtime evidence exists.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-host-channel-selection.mjs`
- `node scripts/verify/verify-blog-comments-host-provider-selection.mjs`
- `node scripts/verify/verify-blog-comments-tcp-listener-lifecycle.mjs`
- `node scripts/verify/verify-blog-comments-tcp-channel-seam.mjs`
- `node scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`
- `cargo test -p rustok-server --features mod-blog --lib comments_provider_runtime::tests`
- `cargo check -p rustok-server --features mod-blog --locked`

## Boundaries retained

- Comments owns typed envelopes, framing, channel traits, credentials, trusted
  principal replacement, provider dispatch, and stable transport errors.
- The server host owns connector/acceptor selection, runtime-extension
  precedence, listener policy, authority composition, provider selection,
  concurrency, and shutdown.
- A concrete protected-channel implementation owns cryptographic negotiation,
  peer verification, and handshake bounds. Merely reporting
  `AuthenticatedEncrypted` is not runtime proof.
- Blog owns authenticated user/moderation contexts, article rendering, and
  degraded presentation through the transport-neutral Comments port.
- Plaintext and externally protected profiles remain loopback-only until retained
  execution evidence justifies a separate non-loopback slice.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
