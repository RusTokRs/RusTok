# rustok-events-module

## Purpose

`rustok-events-module` is the cycle-free RusToK runtime and manifest adapter
for the event capability. Canonical event contracts remain in
`rustok-events`; delivery persistence remains in `rustok-outbox`.

## Responsibilities

- Register the required `events` core module and its dependency on `outbox`.
- Own the Events module manifest and module-owned Leptos/Next admin packages.
- Preserve the dependency direction when `rustok-core` consumes event
  contracts.
- Delegate concrete runtime readiness to the host event runtime.

## Interactions

- Depends on `rustok-core` for module registration.
- Composes the contract owner `rustok-events` and the delivery owner
  `rustok-outbox` without moving either responsibility into this adapter.
- Publishes no migrations of its own.

## Entry points

- `EventsModule`
- `rustok-module.toml`
- `admin/`
- `next-admin/`

See the [local module contract](docs/README.md), the
[adapter implementation plan](docs/implementation-plan.md), and the
[canonical Events capability plan](../rustok-events/docs/implementation-plan.md).
