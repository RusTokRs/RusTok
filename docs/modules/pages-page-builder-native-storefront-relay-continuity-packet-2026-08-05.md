# Pages / Page Builder Native Storefront Relay Continuity Packet

Date: 2026-08-05
Status: source-ready / execution-pending / FFA-FBA-not-promoted
Scope: reviewed Fly publication → durable lifecycle outbox → real relay → Pages generation rotation → admitted registered native storefront miss/refill/hit

## Rechecked cursor

PR #2992 retained the shared Page Builder → Pages → native storefront seam through a real reviewed publication, immutable materialization evidence, channel-constrained artifact selection and integrity-before-cache-fill ordering.

The remaining gap was temporal continuity. The reviewed publish transaction wrote durable lifecycle events, and the registered route consumed generation-aware cache state, but no retained packet connected those owners in one process and one revision.

This slice closes that source cursor without changing production behavior.

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

## Durable event sequence

The owner operations retain three root outbox envelopes in commit order:

1. draft creation writes `NodeCreated`;
2. reviewed publication writes `NodeUpdated`;
3. the same reviewed transaction writes `NodePublished` before its durable receipt and commit complete.

The real `OutboxRelay` is configured with a one-row batch and one delivery worker so each boundary can be observed separately.

`NodeCreated` is delivered and acknowledged first. The Pages cache handler intentionally ignores it, so route/page/artifact generations remain `3/5/7` and no invalidation request or receipt is produced.

`NodeUpdated` is delivered second. The real Pages handler maps it to the mutable scope set only, advancing route/page generations to `4/6` while artifact remains `7`. Its request and receipt retain the exact event UUID as both event and root correlation identity.

## Fill between NodeUpdated and NodePublished

Before `NodePublished` is delivered, the admitted `web` request calls the real registered endpoint. It misses and fills a composite key bound to generations `4/6/7` and returns the reviewed immutable artifact URL:

```text
/api/pages/{page_id}/artifact?locale=en&channel=web
```

The first key and response remain recorded.

## NodePublished rotation

The relay then claims and delivers the durable `NodePublished` row to `PageCacheInvalidationEventHandler`.

The resulting request is bound to:

- the published event UUID;
- the same root correlation UUID;
- the exact tenant and page;
- the `Published` cause;
- route, page and artifact scopes.

The validated receipt advances generations from `4/6/7` to `5/7/8`. Only after the target returns success does `OutboxRelay` mark the row dispatched and clear claim/error/retry state.

## Old key retention and new-key refill

Generation rotation does not scan or delete cache values. The key created under `4/6/7` remains physically present.

The next identical admitted native request derives a different key under `5/7/8`, misses, re-reads the verified reviewed artifact, fills the new key with the production storefront TTL and returns the same exact artifact URL and response body.

A third identical request reads the new key and does not perform another put. The final recording state contains both old and new keys, while only the new generation-bound key is reachable.

## Production ordering retained

The source verifier locks the existing owner boundaries:

1. reviewed compilation and immutable binding precede `NodeUpdated`/`NodePublished`, receipt insertion and commit;
2. `NodeUpdated` maps only to route/page scopes;
3. `NodePublished` maps to route/page/artifact scopes;
4. relay target publication precedes durable `mark_dispatched`;
5. native channel admission precedes generation/key/cache work;
6. owner page and immutable artifact verification precede cache fill.

## Evidence boundary

Machine evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-native-storefront-relay-continuity-source.json`;
- `crates/rustok-pages/storefront/tests/native_storefront_relay_continuity_sqlite.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs`.

The execution list is empty and every validation flag remains false. Tests, Cargo, formatting, verifier execution, SQLite/Axum, Leptos server functions, workflows and CI were not run.

## Deliberate limits

This slice does not:

- change production Pages, Page Builder, Outbox, Channel, cache or route behavior;
- alter event ordering, scopes, generations, key shape, TTL or fail-open policy;
- add cache scans, wildcard deletion or another provider;
- change production migrations, entities, DTOs or public transports;
- exercise a relay failure/restart path, which remains covered by the separate PostgreSQL source packet;
- claim PostgreSQL, browser, workflow, CI or observed tenant evidence;
- promote FFA or FBA status.

## Remaining cursor

The source pipeline is now continuous through the registered native route. The next work should shift from adding another source seam to executing and retaining the existing packets, beginning with the focused SQLite/Axum native route set and its static verifiers. PostgreSQL relay/cache, metadata conflict/isolation, published browser and rollout evidence remain required before promotion.

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
