# rustok-index-admin

Leptos admin UI package for the `rustok-index` module.

## Responsibilities

- Exposes the index module overview used by `apps/admin`.
- Keeps index-specific operator visibility inside the module package.
- Participates in the manifest-driven admin UI composition path through `rustok-module.toml`.
- Uses native Leptos `#[server]` functions for the bootstrap surface.

## Entry Points

- `IndexAdmin` - root admin page component for the module.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/index`.
- Reads tenant-scoped index counters directly from the server runtime instead of going through GraphQL.

## Documentation

- See [platform docs](../../../../docs/index.md).
