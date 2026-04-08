# rustok-outbox

## Purpose

`rustok-outbox` owns the canonical outbox transport and relay pipeline for reliable event delivery in RusToK.

## Responsibilities

- Persist outbound events through the shared outbox transport.
- Relay pending events with claim, dispatch, retry, and DLQ semantics.
- Own the `sys_events` schema and related migrations.
- Expose the runtime services used by `apps/server` event bootstrap and background delivery.
- Ship the module-owned Leptos admin UI package for relay visibility.

## Entry points

- `OutboxModule`
- `OutboxTransport`
- `OutboxRelay`
- `migration`

## Interactions

- Depends on `rustok-core` for event contracts and transport abstractions.
- Used by `apps/server` for runtime relay wiring, background processing, and migrations.
- Integrates with target transports such as `rustok-iggy` instead of owning transport-specific adapters inline.
- The Leptos admin UI lives in `crates/rustok-outbox/admin` and is mounted through manifest-driven host wiring.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
