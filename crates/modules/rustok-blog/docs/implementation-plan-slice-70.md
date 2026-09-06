# rustok-blog implementation plan — slice 70 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-69.md`. Slices 1–66 remain in
the original plan; slices 67–69 retain the typed remote core, TCP client, and
trusted accepted-stream server adapter.

## 2026-07-31 continuation audit

Slice 69 was rechecked against current `main` at
`f0bcd089970aa3b03bbde38fb45c61a68abe29cb`. The shared bounded framing,
mandatory authority seam, seven-operation dispatch, exact provider error reply,
and explicit runtime non-claims remain present.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 70 — host-selected Comments provider publication

### Implemented source scope

- `apps/server/src/services/comments_provider_runtime.rs` adds the server-owned
  Comments provider selector.
- `RUSTOK_COMMENTS_PROVIDER_MODE` accepts `in_process` or `tcp`; absence defaults
  to `in_process`.
- `in_process` intentionally publishes no `Arc<dyn CommentsThreadPort>`. Existing
  Blog GraphQL, Axum HTTP, server-function, admin-native, and storefront-native
  consumers therefore retain their current database/event-bus fallback.
- `tcp` requires `RUSTOK_COMMENTS_TCP_ENDPOINT` as an explicit IP socket address.
- Because the current transport is plaintext, TCP publication is limited to a
  loopback endpoint. Non-loopback addresses fail startup configuration closed.
- TCP mode constructs `TcpJsonCommentsTransport`, wraps it through
  `remote_comments_thread_port`, and inserts the resulting
  `Arc<dyn CommentsThreadPort>` into `ModuleRuntimeExtensions`.
- A provider already present in `ModuleRuntimeExtensions` is preserved and
  classified as `Preconfigured`; the selector never replaces an externally
  composed host adapter.
- `CommentsProviderRuntimeSelection` records only the selected profile and
  optional socket address for typed host introspection.
- `apps/server/src/services/mod.rs` invokes the selector once during the existing
  host-provider extension composition. `ModuleRuntimeExtensions::apply_to_host_runtime`
  then publishes the same selected port into GraphQL and server-function/Axum
  host snapshots without separate transport-specific wiring.
- `crates/rustok-distribution/Cargo.toml` enables the existing
  `rustok-comments/tcp-transport` feature whenever the Comments distribution
  feature is selected.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-host-provider-selection.json`.
- The standalone fail-closed verifier is
  `scripts/verify/verify-blog-comments-host-provider-selection.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- TCP listener bind or accept-loop lifecycle;
- a concrete server authority resolver, credential loader, mTLS identity mapper,
  or encrypted transport;
- DNS/service discovery or non-loopback TCP publication;
- connection idle timeout, concurrency limits, or graceful listener shutdown;
- retry/backoff, circuit breaking, or replay policy;
- an active TCP connection or provider health probe during startup;
- in-process/TCP parity execution;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add host-owned listener lifecycle with bounded concurrency, pre-request idle
   timeout, graceful shutdown, and a concrete authenticated authority resolver.
2. Replace the loopback-only plaintext publication boundary with mTLS or an
   equivalently authenticated and encrypted transport before allowing non-loopback
   endpoints.
3. Add bounded retry/backoff only for retryable failures. Writes must never be
   replayed without the existing idempotency key, and the original deadline must
   bound the complete attempt set.
4. Add in-process/TCP parity fixtures for all seven operations, selection modes,
   authority denial, tenant mismatch, exact provider errors, malformed requests,
   oversized frames, disconnects, and timeouts.
5. Run and record retained Rust, JavaScript, TCP, PostgreSQL, browser, workflow,
   and CI targets before promoting evidence beyond source-only status.
6. Continue cached thread snapshot and comment-form fallback work after the
   remote path has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-host-provider-selection.mjs`
- `node scripts/verify/verify-blog-comments-tcp-server-adapter.mjs`
- `cargo test -p rustok-server --features mod-blog --lib comments_provider_runtime::tests`
- `cargo check -p rustok-server --features mod-blog`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns storage, moderation lifecycle, public projection, typed remote
  requests/replies, provider dispatch, and concrete client/server adapters.
- Blog owns consumer policy, article rendering, typed degraded presentation, and
  transport-neutral `Arc<dyn CommentsThreadPort>` consumption.
- The server host owns deployment selection, endpoint policy, listener lifecycle,
  authentication, transport protection, and publication into shared runtime
  snapshots.
- Default configuration must preserve in-process behavior. An invalid explicit TCP
  configuration must fail startup rather than silently fall back.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
