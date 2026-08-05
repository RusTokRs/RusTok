# Pages / Page Builder Native Storefront Server Function Packet

Date: 2026-08-05
Status: source-ready / SQLite-Axum-execution-pending / FFA-FBA-not-promoted
Scope: registered Leptos Pages server function → trusted tenant/request extraction → real Pages owner read → composite cache miss/refill/hit → generation rotation → fail-open owner read

## Cursor closed by this source slice

The preceding native storefront cache packet retained the public `PagesCacheReadRuntime` and composite key contract, but it did not mount the registered Leptos endpoint. The next open cursor was the real `/api/fn/pages/storefront-data` route with host context and real Pages owner data.

This slice adds that retained source packet. It does not claim that Cargo, the test, the verifier, SQLite, Axum, the server function, a browser, a workflow or CI ran.

## Registered route harness: ready, unvalidated

`crates/rustok-pages/storefront/tests/native_storefront_server_fn_sqlite.rs` links the production `rustok-pages-storefront` crate so the `#[server(prefix = "/api/fn", endpoint = "pages/storefront-data")]` registration is present. It mounts the same wildcard Axum shape used by the server host:

```text
POST /api/fn/{*fn_name}
  -> handle_server_fns_with_context
  -> provide_context(HostRuntimeContext)
  -> registered pages/storefront-data server function
```

Requests target `/api/fn/pages/storefront-data` with the default form codec. A real `TenantContextExtension` is attached to the request; the production adapter derives `RequestContext` through `leptos_axum::extract` rather than accepting a test-only request DTO.

## Real owner fixture

The harness uses an isolated in-memory SQLite database and applies:

1. the real `SysEventsMigration`;
2. every real `PagesModule` migration.

It constructs a real `TransactionalEventBus`, creates a localized Pages draft through `PageService::create`, and publishes it through `publish_non_builder_if_current`. The fixture deliberately uses a non-builder HTML body so this packet isolates the registered server-function, owner-read and composite-cache boundary. It does not duplicate the immutable Page Builder artifact fixture already retained by the public artifact HTTP packet.

## Retained route observations

### Initial miss and refill

With route/page/artifact generations `3/5/7`, the first registered server-function request returns the owner body `source-v1`, records one composite-key miss and fills the same key with `PAGES_STOREFRONT_CACHE_TTL_SECS`.

### Same-generation hit before owner refresh

The durable owner body is changed to `source-v2` without rotating generations. The second route request returns the exact first response and does not add another cache write. This proves the registered endpoint reaches the typed cache hit before refreshing the owner page.

### All-generation rotation

Route/page/artifact generations advance to `4/6/8`. The next request constructs a different key, reads `source-v2` from the owner and fills the new key. The old-generation value remains physically present.

### Cache-read failure

The owner body advances to `source-v3`, generations advance to `5/7/9`, and the cache port returns a read failure. The registered route still returns the authoritative owner body and performs a best-effort refill with the production TTL.

### Generation-read failure

The owner body advances to `source-v4` and generation loading fails. The route bypasses both lookup and fill, invents no default key and still returns the authoritative owner body.

## Production source retained

The production native adapter remains unchanged. It still:

- resolves trusted tenant, locale and optional channel context;
- checks routed-channel module admission before cache lookup when a channel is present;
- binds route, requested locale, fallback locale and resolved channel into the variant;
- uses current route/page/artifact generations in the Pages-owned composite key;
- returns a typed hit before owner page/list reads;
- fills only after the complete owner response;
- fails open on generation, read and fill failures.

The server host also remains unchanged and continues to mount `/api/fn/{*fn_name}` with `handle_server_fns_with_context` and the composed `HostRuntimeContext`.

## Evidence

Machine evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-native-storefront-server-fn-source.json`;
- `crates/rustok-pages/storefront/tests/native_storefront_server_fn_sqlite.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs`.

The execution list is empty and every validation flag is false.

## Deliberate limits

This slice does not:

- change production Pages, Page Builder, cache or server-function behavior;
- change migrations, entities, DTOs, routes, codecs, namespaces, keys or TTL;
- execute routed-channel module admission;
- execute the native route with a Fly/Page Builder artifact body;
- connect a durable relay delivery to the route in one process;
- run PostgreSQL, a browser, workflow checks or CI;
- promote FFA or FBA status.

## Remaining cursor

1. Run and retain this SQLite/Axum registered-route packet.
2. Add a routed-channel packet with real channel/module admission before lookup.
3. Add a registered native-route fixture using a verified immutable Page Builder artifact.
4. Connect a durable `NodePublished` relay delivery to generation rotation and the next native route miss/refill on one exact revision.
5. Execute existing metadata, artifact HTTP, PostgreSQL outbox/cache and relay-restart packets before promotion.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_server_fn_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-pages --all-targets
```
