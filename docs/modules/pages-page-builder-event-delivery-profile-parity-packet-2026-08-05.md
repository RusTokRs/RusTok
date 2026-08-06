# Pages / Page Builder event delivery profile parity packet

Date: 2026-08-05
Status: source-ready / execution-pending

## Purpose

Retain one server integration source that constructs the real event runtime through `build_event_runtime` for `outbox_local` and proves that the production Pages generation gate preserves its durable delivery boundary. The packet also records the `outbox_iggy` factory boundary without requiring external Iggy infrastructure.

Earlier packets exercised the gate through manually assembled relay targets. This packet covers the factory-selected topology itself.

## OutboxLocal profile

```text
RustokSettings.events.delivery_profile = OutboxLocal
  → build_event_runtime
application publish
  → OutboxTransport
  → durable Pending sys_events row
  → no Pages rotation
  → no listener_bus delivery
OutboxRelay
  → ArtifactEventProjectionTransport
  → TenantGenerationDeliveryGate
  → ServerPagesCachePort rotation
  → local MemoryTransport listener delivery
  → durable Dispatched acknowledgement
ordinary Pages listener
  → same-event duplicate no-op
```

The retained source requires `ReliabilityLevel::Outbox`, a real relay configuration, a pending row before relay, generations still at `0/0/0`, and a silent listener bus before relay processing. After one real relay pass, generations are `1/1/1`, the same event/correlation identity reaches the listener, and the row is `Dispatched` with cleared claim/error scheduling fields.

## OutboxIggy boundary

The production factory continues to compose `OutboxIggy` through the same artifact projection and tenant generation transport before `OutboxRelay` acknowledgement, with local listener fan-out after primary Iggy acceptance. This packet does not instantiate Iggy, start a bundled broker, connect to external infrastructure, or claim OutboxIggy execution.

## Source retained

- `apps/server/tests/pages_event_delivery_profiles_sqlite.rs`
- `crates/rustok-pages/contracts/evidence/pages-event-delivery-profile-parity-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-event-delivery-profile-parity.mjs`

The harness uses isolated SQLite databases with `rustok_migrations::Migrator`, initializes the shared production `CacheService` before runtime construction, and uses the real factory, relay, gate and `ServerPagesCachePort`.

## Boundaries

This slice does not:

- change production Pages, Page Builder, event factory, Outbox, Iggy or cache code;
- change dependencies, migrations, schemas, DTOs, routes, cache namespaces, key shape or TTL;
- start the outbox relay supervisor loop;
- execute an external Iggy deployment;
- claim tests, Cargo, formatting, verifiers, SQLite, runtime profiles, workflows or CI were run;
- promote Pages FFA or Page Builder FBA.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-event-delivery-profile-parity.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs

cargo test -p rustok-server --features mod-pages \
  --test pages_event_delivery_profiles_sqlite -- --nocapture
cargo check -p rustok-server --features mod-pages --all-targets
```

Execution evidence remains pending.
