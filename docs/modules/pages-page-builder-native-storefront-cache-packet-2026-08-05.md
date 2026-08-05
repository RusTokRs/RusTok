# Pages / Page Builder Native Storefront Cache Packet

Date: 2026-08-05
Status: source-ready / execution-pending / FFA-FBA-not-promoted
Scope: native Pages storefront composite response → generation snapshot → composite key → miss/refill/hit → generation rotation → fail-open source read

## Rechecked cursor

The parity line was rechecked against current `main` and the recent merged Pages slices:

- PR #2971 added the source-ready PostgreSQL publish/rollback outbox-to-cache packet;
- PR #2974 added the source-ready durable relay failure/restart packet;
- PR #2979 added the source-ready SQLite/Axum public artifact HTTP cache packet.

Those slices already cover durable event correlation, relay recovery and the public immutable artifact route. The next uncovered source boundary was the native storefront composite response that uses route, page and artifact generations together.

## Packet added by this slice

`crates/rustok-pages/tests/native_storefront_cache_contract.rs` adds one bounded contract harness over the same public cache primitives used by the native server function:

- `PagesCacheReadPort`;
- `PagesCacheReadRuntime`;
- `PageCacheGenerationSnapshot`;
- `storefront_pages_cache_key`;
- the production Pages storefront TTL.

The harness does not introduce a second cache policy or a replacement production reader. It records the public contract while the verifier source-locks the real native adapter ordering.

### Initial miss and refill

With route/page/artifact generations `3/5/7`, the first read:

1. loads the generation snapshot;
2. builds the composite storefront key;
3. misses the cache;
4. calls the bounded source closure once;
5. fills the same key with the production Pages storefront TTL.

### Same-generation hit

The second read uses the same variant and generations. It returns the cached snapshot and does not call the source closure again or write another value.

### Generation rotation

The harness advances route/page/artifact generations to `4/6/8`. The next read constructs a different key, misses, loads a new source snapshot and refills the new key. The old-generation value remains physically present but is not reachable from current key construction.

### Cache-read failure

The harness then advances generations to `5/7/9` and injects a cache read failure. The authoritative source closure still runs and the result is eligible for best-effort refill. This retains the production fail-open policy: cache infrastructure does not become storefront data authority.

### Generation-read failure

A generation snapshot failure bypasses both cache lookup and cache fill. The source closure still returns the storefront data. No key is invented from stale or default generations.

## Production source ordering retained

The real native adapter remains unchanged and keeps this order:

1. resolve trusted tenant, locale and channel context;
2. require the Pages module for the routed channel;
3. derive a bounded route/locale/fallback/channel variant;
4. read the current route/page/artifact generation snapshot;
5. build the Pages-owned composite key;
6. return a typed cache hit before owner reads;
7. on miss or cache error, load the published page and verified immutable artifact through Pages owners;
8. load the public page list;
9. fill the cache only after the owner result is complete;
10. ignore cache fill failure and return the authoritative source result.

The source verifier is `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs`.

## Evidence boundary

Machine evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-native-storefront-cache-source.json`;
- `crates/rustok-pages/tests/native_storefront_cache_contract.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs`.

The execution list is empty and every validation flag remains false. Tests, Cargo, formatting, verifiers, database scenarios, the native server function, browser flows, workflows and CI were not run in this slice.

## Deliberate limits

This slice does not:

- change production Pages or Page Builder behavior;
- add a cache provider, Redis command, wildcard scan or key deletion;
- alter cache namespaces, key shape, TTL or failure policy;
- change migrations, entities, DTOs, GraphQL, HTTP or server-function routes;
- claim a real tenant, database, Leptos/Axum request or browser observation;
- promote FFA or FBA status.

## Remaining cursor

native server-function execution remains pending. The next packet should mount the real registered Leptos server-function route with `HostRuntimeContext`, trusted tenant/request context, a real Pages database fixture and the typed cache runtime, then retain miss/refill/hit and generation-rotation observations without bypassing module or channel admission.

A later continuity packet should connect a real durable `NodePublished` relay delivery to that native storefront request on one exact revision. Executed database, HTTP/browser, workflow and rollout evidence still blocks promotion.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
cargo test -p rustok-pages --test native_storefront_cache_contract -- --nocapture
cargo check -p rustok-pages --all-targets
cargo check -p rustok-pages-storefront --features ssr --all-targets
```
