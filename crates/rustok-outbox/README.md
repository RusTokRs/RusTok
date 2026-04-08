# rustok-outbox

## Purpose

`rustok-outbox` owns transactional event persistence and relay infrastructure for RusTok.

## Responsibilities

- Provide `OutboxModule` metadata for the runtime registry.
- Persist domain events into `sys_events` through `OutboxTransport`.
- Relay pending events with claim, dispatch, retry, and DLQ handling through `OutboxRelay`.
- Own the `sys_events` schema migration and baseline relay metrics.
- Expose a transactional event bus abstraction for server-side orchestration.

## Interactions

- Depends on `rustok-core` for `EventTransport`, envelopes, and module contracts.
- Used by `apps/server` for migrations, runtime relay bootstrap, and event transport wiring.
- Can forward claimed events to downstream transports such as `rustok-iggy` without owning provider-specific delivery semantics.
- Publishes a module-owned Leptos admin package at `crates/rustok-outbox/admin` for relay visibility in host UIs.

## Entry points

- `OutboxModule`
- `OutboxTransport`
- `OutboxRelay`
- `RelayConfig`
- `RelayMetricsSnapshot`
- `TransactionalEventBus`
- `SysEventsMigration`
- `SysEvents`
- `SysEvent`

See also [docs/README.md](docs/README.md).
