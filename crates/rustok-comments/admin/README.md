# rustok-comments-admin

Leptos admin UI package for the `rustok-comments` module.

## Responsibilities

- Exposes the module-owned moderation/operator surface used by `apps/admin`.
- Uses `src/transport/mod.rs` and `src/transport/native_server_adapter.rs` as the only internal data layer for comments moderation; the pre-FFA `src/api.rs` facade is intentionally absent.
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
