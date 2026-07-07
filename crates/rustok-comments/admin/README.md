# rustok-comments-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-comments` module.

## Responsibilities

- Exposes the module-owned moderation/operator surface used by `apps/admin`.
- Uses `src/transport/mod.rs` and `src/transport/native_server_adapter.rs` as the only internal data layer for comments moderation; the pre-FFA `src/api.rs` facade is intentionally absent.
- Consumes host-provided `rustok_api::HostRuntimeContext` for native DB access and must not depend on Loco `AppContext`.
- Does not introduce a new GraphQL or REST transport just for parity; `rustok-comments` has no legacy transport surface.
- Participates in manifest-driven admin composition through `rustok-module.toml`.

## Entry Points

- `CommentsAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses `rustok-comments::CommentsService` only inside the native server-function adapter path.
- Preserves existing integrations such as `rustok-blog -> rustok-comments`.

## Documentation

- See [platform docs](../../../docs/index.md).
