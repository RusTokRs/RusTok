# rustok-events-admin

Module-owned Leptos admin surface for event delivery profile control.

## Responsibilities

- Exposes the event administration root view used by `apps/admin`.
- Provides delivery profile configuration and event stream status views.
- Implements Fluid Frontend Architecture (FFA) decoupling core mapping from UI views.
- Participates in manifest-driven UI composition through `rustok-module.toml`.

## Entry Points

- `EventsAdmin` — root admin component for the events module.

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.
- See [platform docs](../../../../docs/index.md).
