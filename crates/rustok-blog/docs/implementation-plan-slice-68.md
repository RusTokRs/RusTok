# rustok-blog implementation plan — slice 68 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-67.md`. Slices 1–66 remain in
the original implementation plan, and slice 67 remains the retained typed remote
adapter-core record.

## 2026-07-31 continuation audit

The slice 67 remote adapter core was rechecked against current `main` at
`d5b79c98cbd5d7f00e88e0587dacfe3c91cea06e`. The seven-operation request and
response model, complete `PortContext`, read/write policy enforcement, Blog host
injection seams, typed storefront degradation, source evidence, and explicit
runtime non-claims remain present.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 68 — concrete TCP JSON Comments transport

### Implemented source scope

- `crates/rustok-comments/src/tcp_transport.rs` adds
  `TcpJsonCommentsTransport`, a concrete implementation of
  `CommentsThreadTransport`.
- The transport uses the source-owned protocol identity
  `comments_tcp_length_prefixed_json_v1`: a four-byte unsigned big-endian length
  followed by one JSON request, then one length-prefixed JSON reply.
- Each connection carries exactly one `CommentsThreadRequest` and one
  `CommentsThreadTransportReply`, then closes.
- `CommentsThreadTransportReply` preserves either a typed
  `CommentsThreadResponse` or the exact provider `PortError`.
- Every request exposes its complete context through
  `CommentsThreadRequest::context()` without reconstructing or dropping tenant,
  actor, claims, roles, channel, locale, correlation, causation, trace,
  idempotency, or deadline fields.
- The transport requires deadline semantics before connecting and wraps connect,
  write, read, and decode in one `tokio::time::timeout` using
  `PortContext.deadline_ms`.
- Request and response frames are bounded by an eight-MiB default, configurable
  only inside `1..=u32::MAX`.
- Invalid limits, oversized frames, encode failures, decode failures, transport
  unavailability, and deadline exhaustion remain typed and fail closed.
- The `tcp-transport` feature keeps Tokio transport dependencies outside the
  default Comments provider build.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json`.
- The standalone fail-closed verifier is
  `scripts/verify/verify-blog-comments-tcp-transport.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- a TCP server adapter or listener;
- endpoint discovery, DNS policy, or configuration loading;
- authentication, credentials, or transport encryption;
- retry_backoff, circuit breaking, or replay policy;
- host_publication or automatic in-process/remote selection;
- in-process/remote parity execution;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add a `tcp_server_adapter` that decodes the same bounded request frame,
   dispatches to a host-owned `Arc<dyn CommentsThreadPort>`, and returns the
   stable typed reply envelope.
2. Publish the client and server adapters through host runtime configuration,
   preserving the current in-process fallback and keeping endpoint/auth ownership
   outside Blog.
3. Add bounded retry_backoff only for retryable failures. Writes must never be
   replayed without the existing idempotency key, and the original deadline must
   bound the complete attempt set.
4. Add in-process/TCP parity fixtures for all seven operations, provider errors,
   mismatched response variants, oversized frames, disconnects, and timeout.
5. Run and record retained Rust, JavaScript, TCP, PostgreSQL, browser, workflow,
   and CI targets before promoting evidence beyond source-only status.
6. Continue cached thread snapshot and comment-form fallback work after the
   remote path has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-transport.mjs`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_transport::tests`
- `cargo check -p rustok-comments --features tcp-transport`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns storage, moderation lifecycle, public projection, typed remote
  requests/replies, and concrete provider-side transport adapters.
- Blog owns consumer policy, article rendering, typed degraded presentation, and
  host-selected `Arc<dyn CommentsThreadPort>` composition seams.
- The host owns endpoint selection, authentication, service lifecycle, and the
  decision to publish an in-process or remote provider.
- A transport failure must remain a typed `PortError`; Blog must not infer
  Comments storage state from TCP details.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
