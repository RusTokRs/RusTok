# rustok-blog implementation plan — slice 74 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-73.md`. Slices 1–66 remain in
the original plan; slices 67–73 retain the typed Comments remote core, bounded
TCP framing and lifecycle, bearer-authenticated reads, signed user delegation,
and process-local replay admission.

## 2026-08-01 continuation audit

Slice 73 was rechecked against current `main` at
`fd27c7665f61fdacd3e8ebfa677d0569f87e05c6`. No later main commit existed when
this continuation branch was created.

The next planned result was a TLS/mTLS transport. The current framing and
adapter implementations, however, owned concrete `tokio::net::TcpStream`
values. Adding certificate configuration directly there would couple typed
Comments protocol logic to one TLS library and make later channel testing,
rotation, or alternative protected transports unnecessarily invasive.

This bounded continuation therefore introduces the transport-owned protected
channel injection contract first. It deliberately does not claim that TLS or
mTLS is implemented. The concrete rustls client connector, server acceptor,
certificate loading, and host mode are deferred to the next slice.

No compile, Rust or JavaScript test, TCP exchange, TLS handshake, database,
browser, workflow, CI, production, or runtime result is promoted by this
continuation. Execution remains maintainer-owned.

## Slice 74 — protected channel injection core

### Implemented source scope

- `crates/rustok-comments/src/tcp_channel.rs` owns the channel abstraction.
- `CommentsTcpIo` is the byte-stream boundary consumed by framing and typed
  transport code. Implementations must provide asynchronous read/write, be
  unpinned, and be sendable between tasks.
- `BoxCommentsTcpIo` erases the concrete stream type after channel establishment.
- `CommentsTcpChannelProtection` exposes a closed source-level classification:
  - `PlaintextLoopback`;
  - `AuthenticatedEncrypted`.
- The classification is descriptive policy metadata. Selecting
  `AuthenticatedEncrypted` does not itself prove that an implementation is
  secure; a concrete connector/acceptor and retained runtime evidence remain
  required.
- `CommentsTcpClientChannelConnector` owns client channel establishment.
  Implementations may own DNS, SNI, certificate validation, TLS handshakes, or
  another protected-channel policy and return a stream only after that policy
  succeeds.
- `CommentsTcpServerChannelAcceptor` receives the host-accepted raw socket and
  peer address. A TLS/mTLS implementation must complete and independently bound
  its handshake before returning a protected byte channel.
- `PlaintextLoopbackCommentsTcpChannel` preserves the compatibility profile but
  now enforces loopback independently on both client endpoint and accepted peer.
- A non-loopback plaintext endpoint or peer fails with
  `comments.tcp_plaintext_non_loopback` before a typed request is exchanged.
- `crates/rustok-comments/src/tcp_protocol.rs` no longer owns `TcpStream`.
  Length-prefixed framing is generic over asynchronous readers and writers while
  retaining:
  - four-byte big-endian frame length;
  - configured and `u32` frame limits;
  - the existing stable frame and I/O errors.
- `TcpJsonCommentsTransport` stores an injected
  `Arc<dyn CommentsTcpClientChannelConnector>`.
- Existing constructors retain the plaintext loopback compatibility connector.
- New constructors permit a host to combine a connector with:
  - no application credential;
  - a bearer token;
  - bearer plus signed user delegation;
  - an explicit frame limit.
- Connector establishment, typed credential preparation, envelope encoding,
  request write, response read, and response decoding remain inside the original
  `PortContext.deadline_ms` timeout.
- `TcpJsonCommentsTransport::channel_protection` exposes the connector's closed
  protection classification without exposing certificate or secret material.
- Transport `Debug` includes the protection classification and retains redaction
  for bearer and delegation secrets.
- `TcpJsonCommentsServerAdapter` retains its existing plaintext methods for
  compatibility.
- The server adapter adds:
  - `handle_connection_with_acceptor`;
  - `handle_connection_with_acceptor_and_pre_request_timeout`.
- The injected acceptor runs before the adapter reads the first typed frame.
- The existing pre-request timeout begins after channel establishment and bounds
  receipt of the first complete frame. A concrete TLS acceptor must apply a
  separate handshake timeout; this slice does not silently reuse the request
  idle timeout as a handshake bound.
- After channel establishment, protocol-version validation, bearer/delegation
  authority, tenant equality, trusted principal replacement, owner dispatch,
  exact provider replies, request deadlines, and one-request/one-reply scope are
  unchanged.
- No direct dependency was added to `rustok-comments`, and neither its manifest
  nor `Cargo.lock` is changed by this slice.
- The retained slice-68 client and slice-69 server source guards are reconciled
  with the channel abstraction while preserving their historical source-only
  evidence.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-channel-seam.json`.
- The standalone source verifier is
  `scripts/verify/verify-blog-comments-tcp-channel-seam.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- TLS or mTLS handshakes;
- certificate, private-key, CA, DNS name, SNI, ALPN, or revocation validation;
- a rustls, tokio-rustls, native-tls, or OpenSSL adapter;
- encrypted bytes in the built-in host runtime;
- non-loopback endpoint or listener enablement;
- channel-derived Comments authority or certificate-to-principal mapping;
- multi-process, durable, restart-safe, or multi-replica replay prevention;
- key or certificate rotation and secret-manager loading;
- retry/backoff, circuit breaking, or automatic write replay;
- listener health/readiness or successful channel negotiation evidence;
- successful compile, Rust test, JavaScript verifier, TCP/TLS runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add a concrete rustls/tokio-rustls mutual-TLS connector and acceptor using
   host-owned trust roots, certificate chains, private keys, server-name policy,
   TLS 1.3, and a separately bounded handshake.
2. Add fail-closed host configuration for the mTLS client and listener profile.
   Plaintext must remain loopback-only, and non-loopback must remain disabled
   until retained runtime evidence exists.
3. Decide whether verified certificate identity is an additional transport
   gate or the source of service authority. Do not weaken bearer/delegation or
   tenant matching implicitly.
4. Add certificate/key rotation, overlapping trust roots, revocation policy,
   and secret-manager loading without logging key or certificate payloads.
5. Replace the process-local delegation nonce cache with a bounded shared replay
   store before claiming multi-replica or restart-safe replay prevention.
6. Add in-process/plaintext/mTLS parity fixtures for all seven operations,
   handshake failure and timeout, unknown CA, missing client certificate, wrong
   server name, expired certificate, malformed frames, request deadlines,
   concurrency rejection, and shutdown.
7. Add listener readiness and health only after retained runtime execution
   confirms negotiation, application authentication, dispatch, and drain.
8. Continue retry/backoff and cached comment fallback work only after the
   protected remote path has retained runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-channel-seam.mjs`
- `node scripts/verify/verify-blog-comments-tcp-transport.mjs`
- `node scripts/verify/verify-blog-comments-tcp-server-adapter.mjs`
- `node scripts/verify/verify-blog-comments-tcp-user-delegation.mjs`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_channel::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_transport::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_server::tests`
- `cargo check -p rustok-server --features mod-blog --locked`

## Boundaries retained

- Comments owns typed request/reply envelopes, length-prefixed framing, channel
  interfaces, application credentials, trusted principal replacement, provider
  dispatch, and stable transport errors.
- A concrete protected-channel implementation owns cryptographic negotiation,
  peer verification, and handshake bounds. It must not mint Comments authority
  merely by returning `AuthenticatedEncrypted`.
- Blog owns consumer policy, authenticated user/moderation contexts, article
  rendering, and degraded presentation.
- The server host owns connector/acceptor selection, certificate and trust
  configuration, listener policy, authority composition, provider selection,
  concurrency, and shutdown.
- Plaintext remains loopback-only. Non-loopback publication remains forbidden
  until a concrete protected channel and retained runtime evidence are present.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
