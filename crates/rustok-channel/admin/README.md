# rustok-channel-admin

Leptos admin UI package for the `rustok-channel` module.

## Responsibilities

- Exposes the channel admin root view used by `apps/admin`.
- Stays module-owned: channel-specific admin UI does not live in `apps/admin`.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns the experimental channel-management operator flow: bootstrap, create channel, attach targets, bind modules, bind OAuth apps.

## Entry Points

- `ChannelAdmin` — root admin page component for the module.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, and `nav_label` for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/channels` through the generic module page route.
- Uses the thin REST surface exposed by `apps/server/src/controllers/channel.rs`.
- Must keep API assumptions aligned with the `rustok-channel` module and server wiring.

## Documentation

- See [platform docs](../../../../docs/index.md).
