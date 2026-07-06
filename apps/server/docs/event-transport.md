# Event transport in `apps/server`

## What is this

`apps/server` publishes domain events through a shared `EventBus`, and then forwards them to the configured
`EventTransport`.

Flow:

1. Module services (`rustok-content`, `rustok-commerce`, `rustok-forum`, etc.) publish events to `EventBus`.
2. In `apps/server`, a background forwarder starts up that reads events from the shared `EventBus`.
3. The forwarder sends them to the selected transport (`memory | outbox | iggy`).
4. In `outbox` mode, a relay worker is additionally started that unloads events from the outbox.

## Configuration

Configuration is located in `settings.rustok.events`:

- `transport`: `memory | outbox | iggy`
- `relay_interval_ms`: interval for the outbox relay worker
- `iggy`: nested `IggyConfig` configuration

Example:

```yaml
settings:
  rustok:
    events:
      transport: outbox
      relay_interval_ms: 1000
```

Can be overridden via environment variable:

```bash
RUSTOK_EVENT_TRANSPORT=memory
```

If an invalid value is specified, the server fails on startup with an error:

`Invalid RUSTOK_EVENT_TRANSPORT='...' Expected one of: memory, outbox, iggy`

## Where in the code

- settings: `apps/server/src/common/settings.rs`
- transport factory: `apps/server/src/services/event_transport_factory.rs`
- shared event bus and forwarder: `apps/server/src/services/event_bus.rs`
- module-owned listener bootstrap: `apps/server/src/services/module_event_dispatcher.rs`
- runtime connection: `apps/server/src/services/app_runtime.rs`

## Module-owned listeners

`apps/server` no longer maintains separate host-owned dispatchers for `index`,
`search` and `workflow`. Event handlers for these modules are published by the modules
themselves via `RusToKModule::register_event_listeners(...)`, then collected from
`ModuleRegistry` and registered in a single shared `EventDispatcher`.

This path does not include cron/background jobs. For example,
`WorkflowCronScheduler` remains a separate runtime path and is not considered
an `event_listener`.

## Current implementation limitations

- In `outbox` mode, relay currently sends from outbox to `MemoryTransport` (local target by default).
  For production-streaming scenarios, you need to configure a target transport (e.g. Iggy) for the relay chain.
- If transport is not initialized, `EventBus` continues to work in-memory (with warning in logs).
