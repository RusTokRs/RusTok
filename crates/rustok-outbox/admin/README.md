# rustok-outbox-admin

Leptos admin UI package for the `rustok-outbox` module.

## Responsibilities

- Exposes the outbox relay overview used by `apps/admin`.
- Keeps outbox-specific runtime visibility inside the module package.
- Participates in the manifest-driven admin UI composition path through `rustok-module.toml`.
- Uses native Leptos `#[server]` functions for the bootstrap surface.

## Entry Points

- `OutboxAdmin` - root admin page component for the module.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/outbox`.
- Reads outbox relay counters directly from the server runtime instead of going through GraphQL.

## Documentation

- See [platform docs](../../../../docs/index.md).
