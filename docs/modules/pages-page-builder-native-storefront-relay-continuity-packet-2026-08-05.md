# Pages / Page Builder Native Storefront Relay Continuity Packet

Date: 2026-08-05
Status: source-corrected / execution-pending / production-topology-gap-open / FFA-FBA-not-promoted
Scope: reviewed Fly publication → durable lifecycle outbox → real relay with a synchronous test relay target → Pages generation rotation → admitted registered native storefront miss/refill/hit

## Rechecked cursor

PR #2992 retained the shared Page Builder → Pages → native storefront seam through a real reviewed publication, immutable materialization evidence, channel-constrained artifact selection and integrity-before-cache-fill ordering.

PR #2995 connected the owner transaction, durable outbox rows, `OutboxRelay`, the real Pages invalidation handler and the registered native route in one SQLite process. A later topology audit found that the packet described one boundary too strongly: the harness target invokes the Pages handler synchronously, while the production server relay target publishes into the configured transport and the module listener is dispatched separately.

This correction preserves the useful source contract and removes the unsupported production-topology claim.

## Source harness

`crates/rustok-pages/storefront/tests/native_storefront_relay_continuity_sqlite.rs` uses one isolated SQLite database and one shared in-memory cache port.

It applies the real Outbox, Channel and Pages migrations, creates an enabled `web` channel through `ChannelService`, creates a channel-constrained Fly document through `PageService::create`, and publishes it through `PageService::publish_reviewed` with the exact body revision and a valid `PageBuilderReviewedPublishRuntime`.

The durable `page_publish_operations` receipt is reloaded before relay delivery. The registered route is mounted through the production wildcard Leptos server-function handler:

```text
POST /api/fn/{*fn_name}
  → handle_server_fns_with_context
  → provide_context(HostRuntimeContext)
  → pages/storefront-data
```

Trusted tenant and channel extensions are attached to every route request.

## What the relay portion really proves

The harness uses the real `OutboxRelay`, real durable `sys_events` rows and the real `PageCacheInvalidationEventHandler`.

Its `ContinuityTarget` is a synchronous test relay target:

```text
OutboxRelay
  → ContinuityTarget::publish
  → PageCacheInvalidationEventHandler::handle
  → target returns success
  → OutboxRelay::mark_dispatched
```

Therefore the packet proves that, for this test target, handler success precedes durable outbox acknowledgement. It also proves the exact event/correlation identities, scope selection and generation results produced by the real Pages handler.

The harness does not mount the production server relay target. It does not build `EventRuntime`, `MemoryTransport`/Iggy fan-out, the module listener bus or the production `EventDispatcher`.

## Production topology boundary

The current server source composes Outbox delivery differently:

```text
OutboxRelay
  → server relay target
  → local/remote transport acceptance
  → listener_bus
  → module EventDispatcher
  → matching module EventHandler
```

The production module dispatcher remains a separate boundary. It filters handlers through `EventHandler::handles` and runs matching handlers asynchronously. The outbox relay acknowledges after its configured transport target succeeds; this packet does not prove that the later Pages module-listener completion is durably coupled to that acknowledgement.

That distinction matters for crash and listener-failure semantics. The corrected packet must not be used as evidence that production `sys_events.dispatched` means the Pages cache generations were already rotated by the module listener.

## Durable test event sequence

The owner operations retain three root outbox envelopes:

1. draft creation writes `NodeCreated`;
2. reviewed publication writes `NodeUpdated`;
3. the same reviewed transaction writes `NodePublished` before its durable receipt and commit complete.

The real `OutboxRelay` is configured with a one-row batch and one delivery worker so each test-target boundary can be observed separately.

`NodeCreated` reaches the synchronous test target first. The Pages handler produces no invalidation request for it, so route/page/artifact generations remain `3/5/7`.

`NodeUpdated` reaches the handler next. It maps to the mutable scope set only, advancing route/page generations to `4/6` while artifact remains `7`. Its request and receipt retain the exact event UUID as both event and root correlation identity.

## Fill between NodeUpdated and NodePublished

Before the test delivers `NodePublished`, the admitted `web` request calls the real registered endpoint. It misses and fills a composite key bound to generations `4/6/7` and returns the reviewed immutable artifact URL:

```text
/api/pages/{page_id}/artifact?locale=en&channel=web
```

The first key and response remain recorded.

## NodePublished test-target rotation

The relay then claims the durable `NodePublished` row and sends it to the synchronous test target.

The resulting real Pages request is bound to:

- the published event UUID;
- the same root correlation UUID;
- the exact tenant and page;
- the `Published` cause;
- route, page and artifact scopes.

The validated receipt advances generations from `4/6/7` to `5/7/8`. After the synchronous target returns success, `OutboxRelay` marks the row dispatched and clears claim/error/retry state.

This ordering is true for the retained test target. Production listener acknowledgement remains unproven.

## Old key retention and new-key refill

Generation rotation does not scan or delete cache values. The key created under `4/6/7` remains physically present.

The next identical admitted native request derives a different key under `5/7/8`, misses, re-reads the verified reviewed artifact, fills the new key with the production storefront TTL and returns the same artifact URL and response body.

A third identical request reads the new key and does not perform another put. The final recording state contains both old and new keys, while only the new generation-bound key is reachable.

## Production source contracts still retained

The source verifier locks the following independent production boundaries:

1. reviewed compilation and immutable binding precede `NodeUpdated`/`NodePublished`, receipt insertion and commit;
2. `NodeUpdated` maps only to route/page scopes;
3. `NodePublished` maps to route/page/artifact scopes;
4. `OutboxRelay` calls its configured target before `mark_dispatched`;
5. the server outbox profile uses a transport/listener topology distinct from the synchronous test target;
6. the production module dispatcher filters handlers and runs them asynchronously;
7. native channel admission precedes generation/key/cache work;
8. owner page and immutable artifact verification precede cache fill.

These facts do not combine into a claim that the production Pages listener completes before outbox acknowledgement.

## Evidence boundary

Machine evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-native-storefront-relay-continuity-source.json`;
- `crates/rustok-pages/storefront/tests/native_storefront_relay_continuity_sqlite.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs`;
- `docs/modules/pages-page-builder-native-storefront-relay-topology-correction-2026-08-05.md`.

The execution list is empty and every validation flag remains false. Tests, Cargo, formatting, verifier execution, SQLite/Axum, Leptos server functions, production server topology, workflows and CI were not run.

## Deliberate limits

This slice does not:

- change production Pages, Page Builder, Outbox, Channel, cache or route behavior;
- add a synchronous Pages invalidation gate to the production relay target;
- add durable module-listener receipts or listener-to-outbox acknowledgement coupling;
- alter event ordering, scopes, generations, key shape, TTL or fail-open policy;
- add cache scans, wildcard deletion or another provider;
- change production migrations, entities, DTOs or public transports;
- claim PostgreSQL, browser, workflow, CI or observed tenant evidence;
- promote FFA or FBA status.

## Remaining cursor

The owner, handler and route contracts remain source-connected, but production relay-to-listener acknowledgement is open.

The next implementation slice should choose and retain one production-safe ownership model:

1. move Pages generation rotation into a synchronous idempotent relay-target gate and prevent duplicate module-listener rotation; or
2. retain a durable listener receipt/acknowledgement path that keeps the outbox row retryable until Pages invalidation succeeds.

Only after that production topology is source-connected should execution evidence promote the continuity claim.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_relay_continuity_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-pages --all-targets
cargo check -p rustok-page-builder --all-targets
cargo check -p rustok-outbox --all-targets
cargo check -p rustok-channel --all-targets
```
