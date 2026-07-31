# rustok-blog implementation plan — slice 67 continuation

This document is the current continuation of
`crates/rustok-blog/docs/implementation-plan.md`. The original plan remains the
immutable source-history for slices 1–66 and for the existing fail-closed source
guards. This continuation records the re-audit and the next implementation slice
without deleting or rewriting those retained markers.

## 2026-07-31 source re-audit

The recorded Blog work through slice 66 was rechecked against `main` at
`4fa4b790998d6593d24411525b3dbfaf4901bc2c`, together with the current Comments
port, Blog consumer service, HTTP/GraphQL/native composition seams, storefront
availability model, retained evidence, focused verifiers, and explicit
non-claims.

The source artifacts remain present. This audit does **not** promote any compile,
Rust or JavaScript test, PostgreSQL, Redis, browser, workflow, CI, production, or
runtime result. Execution remains maintainer-owned.

## Corrected current status

The previous plan sentence that grouped all degraded UI work as planned is stale.
Typed storefront `AVAILABLE`, `UNAVAILABLE`, and `TIMEOUT` rendering is already
source-locked across GraphQL, native SSR, the shared DTO, and Leptos UI. Only
Comments `ExternalService` and `Timeout` failures degrade to empty typed comment
payloads; other Blog failures remain fail-closed.

The remaining transport gap is narrower:

- Blog already accepts a host-owned `Arc<dyn CommentsThreadPort>` through its
  service and HTTP, GraphQL, storefront-native, and admin-native composition
  seams.
- Slice 67 adds the provider-owned typed remote adapter core.
- A concrete HTTP, gRPC, message-bus, or sidecar client is still pending.
- Endpoint discovery, authentication, retry/backoff, cancellation, and host
  publication are still pending.
- Cached thread snapshots and comment-form fallback are still pending.
- Runtime parity and all execution evidence are still pending.

## Slice 67 — typed Comments remote adapter core

### Implemented source scope

- `crates/rustok-comments/src/remote.rs` owns typed request and response envelopes
  for all seven `CommentsThreadPort` operations.
- `CommentsThreadTransport` is a transport-neutral async dispatch contract.
- `RemoteCommentsThreadPort` implements the existing owner port without changing
  the Blog consumer API.
- Read operations require `PortCallPolicy::read()` before dispatch.
- Write operations require `PortCallPolicy::write()` before dispatch, preserving
  the existing deadline and idempotency-key requirements.
- The complete `PortContext` is carried inside every remote request, including
  tenant, actor, claims, roles, channel, locale, correlation, causation, trace,
  idempotency, and deadline data.
- Response variants are operation-checked. An incompatible response fails closed
  as `comments.remote_response_mismatch` with `InvariantViolation` semantics.
- `rustok-comments` exports the remote adapter through its server feature.
- Source evidence is retained at
  `crates/rustok-blog/contracts/evidence/blog-comments-remote-adapter-core.json`.
- The standalone fail-closed verifier is
  `scripts/verify/verify-blog-comments-remote-adapter-core.mjs`.

### Explicit non-claims

This slice does not implement or claim:

- a concrete network protocol or endpoint;
- service discovery or configuration loading;
- authentication or credential propagation beyond the typed `PortContext`;
- retry, backoff, circuit breaking, transport cancellation, or deadline timers;
- host registration or production selection of the remote adapter;
- parity between in-process and remote execution;
- successful compile, test, verifier, database, browser, workflow, CI, or
  production execution.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add one concrete Comments transport implementation over the typed adapter core.
   Keep endpoint ownership and authentication in the host/transport layer rather
   than the Blog consumer.
2. Publish the remote adapter through the host runtime and generated GraphQL
   attachment inputs while preserving the in-process fallback.
3. Add bounded deadline cancellation and retry policy that cannot replay writes
   without the existing idempotency key.
4. Add in-process/remote parity fixtures for all seven operations and typed
   `PortError` mapping.
5. Run the retained Rust, JavaScript, PostgreSQL, browser, workflow, and CI targets
   before promoting any evidence beyond source-only status.
6. Continue the already-planned cached snapshot and comment-form fallback work
   after the concrete transport has observed runtime evidence.

## Suggested verification — intentionally not run in this slice

- `node scripts/verify/verify-blog-comments-remote-adapter-core.mjs`
- `cargo test -p rustok-comments --features server --lib remote::tests::remote_adapter_accepts_a_transport_trait_object -- --exact`
- `cargo check -p rustok-comments --features server`
- `npm run verify:blog:comments-port-boundary`
- `npm run test:verify:blog:comments-port-boundary`

## Boundaries retained

- Comments owns comment storage, public projection, moderation lifecycle, thread
  invariants, and the `CommentsThreadPort` provider implementation.
- Blog owns the consumer policy, article rendering, typed degraded presentation,
  and host-selected composition seams.
- A remote transport must implement the provider-owned typed transport contract;
  Blog must not reconstruct Comments storage or error classification.
- Source-only evidence must remain explicit until the maintainer runs and records
  the corresponding execution targets.
