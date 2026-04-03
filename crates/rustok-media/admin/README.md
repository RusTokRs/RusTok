# rustok-media-admin

Leptos admin UI package for the `rustok-media` module.

## Responsibilities

- Exposes the module-owned media library surface used by `apps/admin`.
- Uses native Leptos `#[server]` functions as the default internal data layer for list/detail/translations/delete/usage.
- Preserves the existing GraphQL and REST transports in parallel:
  - GraphQL remains the fallback for list/detail/translations/delete/usage.
  - REST remains the upload path.
- Participates in manifest-driven admin composition through `rustok-module.toml`.

## Entry Points

- `MediaAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses `rustok-media::MediaService` directly on the server-function path.
- Keeps the existing `rustok-media` GraphQL and `/api/media` REST contracts intact.

## Documentation

- See [platform docs](../../../docs/index.md).
