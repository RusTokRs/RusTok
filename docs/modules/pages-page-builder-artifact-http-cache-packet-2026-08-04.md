# Pages / Page Builder Artifact HTTP Cache Packet

Date: 2026-08-04
Status: source-ready / SQLite-execution-pending / FFA-FBA-not-promoted
Scope: public Axum artifact request → generation key → cache miss → owner artifact → refill → hit → conditional `304`
Canonical basis:

- `docs/modules/pages-page-builder-parity-continuation-plan.md`;
- `docs/modules/pages-page-builder-postgres-outbox-cache-packet-2026-08-04.md`;
- `docs/modules/pages-page-builder-outbox-relay-restart-packet-2026-08-04.md`.

## Cursor closed by this source slice

The previous packets made publish/rollback outbox correlation and relay restart recovery executable.
The next open cursor was the public artifact HTTP read path after a generation change.

This slice adds one focused SQLite/Axum harness for that exact boundary. It does not claim that the
harness, verifier, Cargo, HTTP server, workflow or CI ran.

## Artifact HTTP cache packet: ready, unvalidated

`crates/rustok-pages/tests/artifact_http_cache_sqlite.rs` applies the real `PagesModule`
migrations to an isolated in-memory SQLite database. It compiles a valid deterministic static
landing artifact through `StaticLandingCompiler`, stores the real Pages page/body/artifact/binding
entities and mounts the public `controllers::axum_router` through `HostRuntimeContext`.

The router receives the existing typed `PagesCacheReadRuntime` and a tenant context extension. No
private controller helper is exposed and no replacement HTTP path is introduced.

### Initial generation miss and refill

The cache begins with artifact generation `7` and no values. The first public request to
`/api/pages/{id}/artifact?locale=en`:

1. returns `200 OK`;
2. reads the generation-bound artifact key;
3. misses the cache;
4. resolves the published owner binding and verifies the compiled artifact;
5. refills the same key with the existing 60-second Pages cache TTL;
6. returns the exact compiled HTML, stable ETag and the existing artifact security headers.

### Old-generation hit and conditional response

The harness deletes the durable published binding after the first refill. A second request carries
`If-None-Match` with the first ETag. It still returns `304 Not Modified` with an empty body and does
not perform another cache fill.

Because the owner binding no longer exists, that successful response can only come from the
old-generation cache value reached by the current generation key.

### Generation advance, miss and refill

The binding is restored and artifact generation advances from `7` to `8`. A third request:

1. constructs a different current key;
2. misses that new key;
3. resolves the restored owner binding;
4. refills the new key;
5. returns `200 OK` with the same immutable artifact and ETag.

The old generation value remains physically present, while current key construction reaches only
the generation-8 value.

### New-generation hit and conditional response

The durable binding is removed again. A fourth request with `If-None-Match` returns `304 Not
Modified` from the generation-8 cache entry without another refill. The recorded cache trace has
four reads and exactly two writes: one for each generation.

## Retained HTTP contract

The focused source packet retains the production response contract:

- `Content-Type: text/html; charset=utf-8` for `200`;
- stable quoted artifact ETag;
- `Content-Language`;
- `Vary: X-Tenant-ID, X-Channel-Slug, X-Channel-ID`;
- `Cache-Control: public, max-age=60, stale-while-revalidate=300`;
- CSS-hash Content Security Policy;
- strict referrer, content-type and cross-origin resource policies;
- empty body for conditional `304`.

## Production boundaries

This slice does not change production Pages, Page Builder, artifact, cache or HTTP behavior. It
adds only:

- one integration-test source;
- one test-only `tower` dependency required for `Router::oneshot`;
- one source evidence contract;
- one static verifier;
- this continuation overlay.

No database migration, entity, DTO, GraphQL surface, public HTTP route, owner service, cache key,
TTL, authorization policy or artifact integrity rule changes.

## Evidence

Source evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-artifact-http-cache-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs`;
- `crates/rustok-pages/tests/artifact_http_cache_sqlite.rs`.

SQLite and Axum execution remain pending. The execution list is empty and every validation flag
remains false.

## Remaining work

The native storefront server-function packet remains open. It should retain the equivalent module
admission, generation snapshot, composite storefront key, miss, owner page/artifact read, refill
and hit ordering, including failure-open behavior when cache reads fail.

A real PostgreSQL/Axum deployment packet and executed relay-to-HTTP continuity packet also remain
open. FFA/FBA promotion stays blocked on executed database, HTTP, browser, workflow and rollout
evidence.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-artifact-http-cache.mjs
cargo test -p rustok-pages --test artifact_http_cache_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```
