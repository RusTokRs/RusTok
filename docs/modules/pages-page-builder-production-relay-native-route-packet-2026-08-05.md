# Pages / Page Builder Production Relay to Native Route Packet

Date: 2026-08-05
Status: source-ready / execution-pending

## Purpose

Retain one source harness that crosses the production server delivery boundary introduced by PR #3001 and reaches the real registered Pages storefront server function.

This packet is deliberately different from the earlier `ContinuityTarget` packet. It mounts the production `TenantGenerationDeliveryGate` and the production `ServerPagesCachePort` rather than a custom synchronous target or a test-owned generation implementation.

## Production components mounted

The retained server integration source uses:

- real `PageService::create` and `PageService::publish_reviewed`;
- real Page Builder reviewed runtime, sanitization, materialization and immutable artifact binding;
- real transactional `NodeCreated`, `NodeUpdated` and `NodePublished` outbox rows;
- real `OutboxRelay` with batch size and concurrency one;
- production `TenantGenerationDeliveryGate`;
- canonical local tenant-generation listener readiness;
- production `PageCacheInvalidationEventHandler`;
- production `ServerPagesCachePort` over one process `CacheService`;
- registered Leptos `/api/fn/pages/storefront-data` route;
- real Channel module admission;
- a recording wrapper that delegates every read and write to the production Pages cache port.

The test-only dependency on `rustok-pages-storefront` exists solely to retain the SSR server-function registration in the server integration target.

## Durable event and route sequence

```text
reviewed page create
  → durable NodeCreated
reviewed publish
  → immutable Fly artifact + durable receipt
  → durable NodeUpdated
  → durable NodePublished
OutboxRelay
  → production TenantGenerationDeliveryGate
  → NodeCreated downstream, no Pages generation change
OutboxRelay
  → production gate
  → NodeUpdated rotates route/page to 1/1, artifact remains 0
registered native route
  → generation snapshot 1/1/0
  → old-key miss
  → verified immutable artifact source read
  → old-key fill
OutboxRelay
  → production gate
  → NodePublished rotates route/page/artifact to 2/2/1
  → downstream acceptance
  → outbox row dispatched
asynchronous Pages listener simulation for the same envelope
  → process-bounded duplicate no-op
registered native route
  → generation snapshot 2/2/1
  → new-key miss
  → same verified immutable artifact source read
  → new-key fill
registered native route
  → new-key hit without another put
```

## Old-key retention

The recording route port exposes the exact old and new composite keys while delegating storage to `ServerPagesCachePort`. After `NodePublished`:

- the old key is still physically readable from the production cache backend;
- generation-aware routing never requests it again;
- the new key differs because route, page and artifact generations changed;
- both responses retain the same reviewed immutable artifact URL.

No scan, wildcard deletion or physical eviction claim is introduced.

## Asynchronous listener compatibility

After the gate has handled the durable `NodePublished` envelope, the harness invokes the normal `PageCacheInvalidationEventHandler` with a separately constructed production provider over the same `CacheService`.

The process-wide stable-event dedupe returns a valid current receipt and keeps generations at `2/2/1`. This represents the later asynchronous module-listener delivery without removing that listener from any delivery profile.

## Boundaries

This slice does not:

- change production Pages, Page Builder, Outbox, Channel, cache or route behavior;
- change cache scopes, namespaces, key shape, TTL or capacity;
- change event, database, DTO, GraphQL, HTTP or server-function schemas;
- claim exact-once invalidation across a process restart;
- mount a full server bootstrap or external Iggy deployment;
- execute tests, Cargo, formatting, verifiers, SQLite, Axum, Leptos, PostgreSQL, browser, workflows or CI;
- promote FFA or FBA status.

The machine evidence execution list remains empty and every validation flag remains false until the maintainer runs the retained commands.

## Retained files

- `apps/server/tests/pages_production_relay_native_route_sqlite.rs`;
- `apps/server/Cargo.toml` test-only SSR dependency;
- `crates/rustok-pages/contracts/evidence/pages-production-relay-native-route-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs`;
- this packet;
- the canonical Pages / Page Builder continuation plan.

## Maintainer validation

```bash
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs
cargo test -p rustok-server --features mod-pages --test pages_production_relay_native_route_sqlite -- --nocapture
cargo check -p rustok-server --features mod-pages --all-targets
```
