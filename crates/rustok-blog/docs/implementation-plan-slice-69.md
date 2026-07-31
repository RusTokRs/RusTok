# rustok-blog implementation plan — slice 69 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-68.md`. Slices 1–66 remain in
the original implementation plan, slice 67 retains the transport-neutral Comments
remote adapter, and slice 68 retains the concrete TCP JSON client transport.

## 2026-07-31 continuation audit

The slice 68 client transport was rechecked against current `main` at
`2d299c82f398b4e0558e7e58bbf19a097462c3c4`. Its seven-operation typed request,
stable success/error reply, bounded length-prefixed JSON framing, complete
`PortContext`, client-side whole-exchange deadline, source evidence, and explicit
runtime non-claims remain present.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 69 — trusted Comments TCP server adapter

### Implemented source scope

- `crates/rustok-comments/src/tcp_protocol.rs` owns the shared bounded four-byte
  unsigned big-endian length-prefix framing used by both TCP client and server.
- `crates/rustok-comments/src/tcp_server.rs` adds
  `TcpJsonCommentsServerAdapter` for exactly one typed request/reply exchange on
  a host-accepted `TcpStream`.
- The adapter decodes `CommentsThreadRequest`, dispatches all seven operations to
  a host-selected `Arc<dyn CommentsThreadPort>`, and returns either the matching
  `CommentsThreadResponse` or the exact provider `PortError` in
  `CommentsThreadTransportReply`.
- Listener ownership, accept-loop lifecycle, concurrency policy, shutdown, and
  socket binding remain host responsibilities.
- `CommentsTcpAuthorityResolver` is mandatory. There is no allow-all constructor
  and no fallback that trusts tenant, actor, claims, or roles from the network
  payload.
- The resolver receives the peer address, stable `CommentsTcpOperation`, and
  claimed context. It returns `TrustedCommentsTcpAuthority` or a typed error.
- The adapter requires the trusted tenant to equal the claimed tenant and then
  replaces actor, claims, and roles with trusted values before provider dispatch.
- Channel, locale, correlation, causation, traceparent, idempotency key, and
  deadline remain request metadata and are preserved when authority is applied.
- Missing deadline semantics fail before authority/provider dispatch. The request
  deadline bounds the combined authority resolution and provider call.
- Malformed typed JSON, authority denial, tenant mismatch, provider errors,
  oversized frames, disconnects, response encoding failure, and processing
  deadline exhaustion remain typed and fail closed.
- `crates/rustok-comments/src/lib.rs` exports the client and server adapters only
  behind the existing opt-in `tcp-transport` feature.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json`.
- The standalone fail-closed verifier is
  `scripts/verify/verify-blog-comments-tcp-server-adapter.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- a listener bind or accept loop;
- a concrete authority resolver, credential loader, mTLS identity mapper, or
  transport encryption;
- endpoint discovery, DNS policy, or runtime configuration publication;
- automatic in-process/TCP provider selection;
- retry/backoff, circuit breaking, replay, or write retry policy;
- connection-level pre-request idle timeout or concurrency limits;
- in-process/TCP parity execution;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Publish the TCP client/server choice through host runtime configuration while
   preserving the current in-process fallback.
2. Add host-owned listener lifecycle, bounded concurrency, shutdown, pre-request
   idle timeout, endpoint configuration, and a concrete authenticated authority
   resolver. Production publication must use transport encryption or an
   equivalently protected local sidecar channel.
3. Add bounded retry/backoff only for retryable failures. Writes must never be
   replayed without the existing idempotency key, and the original deadline must
   bound the complete attempt set.
4. Add in-process/TCP parity fixtures for all seven operations, authority denial,
   tenant mismatch, exact provider errors, malformed requests, oversized frames,
   disconnects, and timeouts.
5. Run and record retained Rust, JavaScript, TCP, PostgreSQL, browser, workflow,
   and CI targets before promoting evidence beyond source-only status.
6. Continue cached thread snapshot and comment-form fallback work after the
   remote path has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-server-adapter.mjs`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_server::tests`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_transport::tests`
- `cargo check -p rustok-comments --features tcp-transport`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns storage, moderation lifecycle, public projection, typed remote
  requests/replies, provider dispatch, and concrete transport adapters.
- Blog owns consumer policy, article rendering, typed degraded presentation, and
  host-selected `Arc<dyn CommentsThreadPort>` composition seams.
- The host owns endpoint and listener lifecycle, authentication, trusted authority
  resolution, transport protection, and in-process/remote selection.
- A transport or authority failure must remain a typed `PortError`; Blog must not
  infer Comments storage state from TCP details.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
