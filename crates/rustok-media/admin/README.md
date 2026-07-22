# rustok-media-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-media` module.

## Responsibilities

- Exposes the module-owned media library surface used by `apps/admin`.
- Keeps the FFA boundary explicit: Leptos-free presentation/form/state helpers live in `src/core.rs` (including busy-key, upload success, detail row, translation form, dimensions and usage card policy), transport calls live in `src/transport/`, and rendering lives in `src/ui/leptos.rs`.
- Uses native Leptos `#[server]` functions as the default internal data layer for list/detail/translations/delete/usage.
- Preserves the existing GraphQL and REST transports in parallel:
  - GraphQL remains the fallback for list/detail/translations/delete/usage.
  - REST remains the upload path.
- Participates in manifest-driven admin composition through `rustok-module.toml`.

## Entry Points

- `MediaAdmin` - root admin view rendered from the host admin registry.
- `src/core.rs` - framework-agnostic admin helpers reused by render adapters.
- `src/transport/` - module-owned native/GraphQL/REST facade with dedicated `native_server_adapter.rs`, `graphql_adapter.rs`, and `rest_adapter.rs` files.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses `rustok-media::MediaService` directly on the server-function path through `HostRuntimeContext`; DB and the host-provided `StorageRuntime` are consumed without a host-wide application context.
- Keeps the existing `rustok-media` GraphQL and `/api/media` REST contracts intact.

## Documentation

- See [platform docs](../../../docs/index.md).
