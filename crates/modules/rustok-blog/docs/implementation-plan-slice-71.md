# rustok-blog implementation plan — slice 71 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-70.md`. Slices 1–66 remain in
the original plan; slices 67–70 retain the typed remote core, TCP client,
trusted accepted-stream adapter, and host-selected consumer publication.

## 2026-08-01 continuation audit

Slice 70 was rechecked against current `main` at
`0eb3b47eef939b93ebbe753a0be1ea8eeff6dcc2`. Default in-process fallback,
explicit loopback-only TCP selection, preservation of preconfigured consumer
ports, shared host publication, source evidence, and runtime non-claims remain
present.

No compile, Rust or JavaScript test, TCP exchange, database, browser, workflow,
CI, production, or runtime result is promoted by this continuation. Execution
remains maintainer-owned.

## Slice 71 — host-owned Comments TCP listener lifecycle

### Implemented source scope

- `apps/server/src/services/comments_provider_runtime.rs` now owns the opt-in
  provider-side listener lifecycle in addition to the consumer selector.
- `RUSTOK_COMMENTS_TCP_LISTENER_ENABLED` defaults to false. Disabled listener
  configuration performs no bind and preserves the existing process behavior.
- An enabled listener requires `RUSTOK_COMMENTS_TCP_BIND` as an explicit,
  non-zero, loopback IP socket address. Plaintext non-loopback bind fails startup
  configuration closed.
- Bounded listener settings are published through:
  - `RUSTOK_COMMENTS_TCP_MAX_CONNECTIONS` (default 64);
  - `RUSTOK_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS` (default 2000);
  - `RUSTOK_COMMENTS_TCP_SHUTDOWN_GRACE_MS` (default 5000);
  - `RUSTOK_COMMENTS_TCP_MAX_FRAME_BYTES` (default 8 MiB and bounded by the
    `u32` wire limit).
- Zero, malformed, non-UTF-8, oversized, missing-required, or unsupported host
  configuration fails startup rather than silently changing mode.
- `SharedCommentsTcpAuthorityResolver` is mandatory when the listener is enabled.
  It may be supplied through `ServerRuntimeContext` or `ModuleRuntimeExtensions`.
  There is no loopback-only allow-all resolver and no fallback that trusts actor,
  claims, roles, or tenant authority from the request payload.
- `SharedCommentsTcpServerProvider` is a distinct provider-side override. It is
  intentionally separate from the consumer-selected `Arc<dyn CommentsThreadPort>`
  so a TCP client cannot be served recursively through itself.
- Without a provider-side override, the host composes the owner-managed in-process
  Comments provider from the server database and transactional event bus.
- The listener uses `TcpJsonCommentsServerAdapter` with the configured frame bound.
- A semaphore rejects connections beyond the configured active-connection limit;
  rejected sockets are closed without provider dispatch.
- Accepted peers are rechecked as loopback before a connection task is spawned.
- `TcpJsonCommentsServerAdapter::handle_connection_with_pre_request_timeout`
  bounds receipt of the complete first request frame and returns
  `comments.tcp_server_idle_timeout` when the peer remains idle.
- After request decode, the request-owned `PortContext` deadline still bounds
  authority resolution and provider dispatch.
- The listener subscribes to the shared server `StopHandle`. Shutdown stops new
  accepts, drains active connection tasks within the configured grace period,
  then aborts only the tasks that remain.
- A lifecycle reservation makes repeated startup calls idempotent and is released
  when configuration, authority resolution, adapter construction, or bind fails.
- `apps/server/src/services/server_bootstrap.rs` starts the listener after app
  runtime composition and before the remaining worker startup path.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json`.
- The standalone fail-closed verifier is
  `scripts/verify/verify-blog-comments-tcp-listener-lifecycle.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- a built-in credential, shared-token, certificate, or mTLS authority resolver;
- a wire credential envelope or encrypted transport;
- non-loopback bind, endpoint discovery, or multi-host publication;
- startup connection probing, listener readiness integration, or provider health
  reporting;
- retry/backoff, circuit breaking, or replay policy;
- in-process/TCP parity execution;
- successful compile, Rust test, JavaScript verifier, TCP runtime, database,
  browser, workflow, CI, or production execution.

The listener cannot start successfully until the host supplies a concrete
`SharedCommentsTcpAuthorityResolver`. This is deliberate: the current wire
protocol carries no credential, so manufacturing a permissive resolver would
create a false authentication claim.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add an authenticated credential envelope or mTLS identity integration and a
   concrete authority resolver. Preserve exact tenant matching and principal-field
   replacement before provider dispatch.
2. Replace loopback-only plaintext constraints with authenticated encryption
   before allowing non-loopback endpoints.
3. Add bounded retry/backoff only for retryable client failures. Writes must never
   be replayed without the existing idempotency key, and the original deadline
   must bound the complete attempt set.
4. Add in-process/TCP parity fixtures for all seven operations, selection and
   listener modes, authority denial, tenant mismatch, malformed requests,
   oversized frames, concurrency rejection, idle peers, disconnects, shutdown,
   and exact provider errors.
5. Add listener health/readiness state only after runtime execution establishes
   bind, drain, and provider behavior.
6. Run and record retained Rust, JavaScript, TCP, PostgreSQL, browser, workflow,
   and CI targets before promoting evidence beyond source-only status.
7. Continue cached thread snapshot and comment-form fallback work after the
   remote path has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-tcp-listener-lifecycle.mjs`
- `node scripts/verify/verify-blog-comments-host-provider-selection.mjs`
- `node scripts/verify/verify-blog-comments-tcp-server-adapter.mjs`
- `cargo test -p rustok-comments --features tcp-transport --lib tcp_server::tests`
- `cargo test -p rustok-server --features mod-blog --lib comments_provider_runtime::tests`
- `cargo check -p rustok-server --features mod-blog`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns storage, moderation lifecycle, public projection, typed remote
  requests/replies, provider dispatch, framing, and connection adapter behavior.
- Blog owns consumer policy, article rendering, degraded presentation, and
  transport-neutral `Arc<dyn CommentsThreadPort>` consumption.
- The server host owns listener configuration and lifecycle, provider-side
  composition, trusted authority supply, endpoint policy, transport protection,
  concurrency, and shutdown.
- Disabled listener configuration must preserve existing behavior. Invalid or
  incomplete explicit listener configuration must fail startup rather than fall
  back to unauthenticated service.
- Source-only evidence remains explicit until the maintainer runs and records the
  corresponding execution targets.
