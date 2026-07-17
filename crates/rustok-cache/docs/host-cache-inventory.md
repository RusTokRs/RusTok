# Host cache contract inventory

This document is the current inventory of active cache instances that participate in the
server/runtime request path. It complements the canonical
[`implementation-plan.md`](./implementation-plan.md): the plan owns priorities and completion
state, while this inventory records the concrete cache contract and the owner of remaining work.

Last reconciled with `main`: 2026-07-16.

## Classification rules

Every active cache must identify:

1. the source of truth;
2. the capacity unit and hard bound;
3. the key compatibility contract;
4. positive and negative freshness policy;
5. local and cross-replica invalidation behavior;
6. degraded-mode behavior;
7. the module that owns recovery when a fast-path invalidation is missed.

A process-local cache is acceptable only when its documented TTL is the approved stale bound or
when the owner has a durable generation/rebuild path. Variable-size values must use byte-weighted
capacity. Fixed-size counters may use entry-count capacity.

## Active production caches

| Cache | Source of truth | Capacity and key contract | Freshness / negative policy | Invalidation and replica contract | Status |
| --- | --- | --- | --- | --- | --- |
| Tenant resolution positive cache | `TenantReadPort` / tenant database state | `CacheService::backend_weighted`; 16 MiB; canonical service/environment/global/domain/schema/resource key with bounded typed identity | 300 s positive TTL with deterministic jitter and bounded loader deadline | Local plus Redis invalidation and shared namespace generation recovery; Redis degradation remains visible while bounded fallback serves eligible reads | Hardened |
| Tenant resolution negative cache | `TenantReadPort` / tenant database state | Separate weighted backend; 1 MiB; separate schema namespace and bounded envelope | Explicit stable `NotFound` / `Disabled` negatives; 60 s TTL and 64 KiB encoded ceiling | Uses the tenant invalidation/generation path; incompatible and expired negatives are removed before reload | Hardened |
| Channel resolution cache | Channel database state through `ChannelResolver`; durable source is `channel_resolution_invalidation_state` | Byte-weighted Moka; 16 MiB; typed key containing tenant, bounded request facts and a monotonic tenant token | 60 s positive TTL; 10 s negative TTL | Mutations to all six channel-owned tables increment one database generation through transaction-local triggers. Successful REST/native writes clear the local tenant token and publish `channel.resolution.generation.v1` as a fast path. Supervised local/Redis/reconcile workers compare the persisted generation every 5 s and perform an atomic namespace rollover when delivery was missed, startup is unverified, or generation regresses. Source evidence covers independent replica readers, two no-Redis runtimes, local queue lag with readiness and resolved-value recovery, PostgreSQL transaction/concurrency/replay, Redis readiness and resolved-value delivery, missed publication, database-state recovery, generation regression and Redis restart/reconnect | Source wiring hardened; compiled PostgreSQL/Redis execution pending |
| Tenant locale cache | `tenant_locales` database rows plus the monotonic shared `tenant.cache.generation.v1` epoch | Byte-weighted Moka; 8 MiB; UUID tenant key; atomic process-local registration | 60 s TTL; empty vectors are ordinary short-lived results rather than a separate negative schema | In-order UUID records invalidate one tenant and `*` invalidates the namespace. Unverified, gapped, lagged or reconciled advancement clears all entries. Every event validates durable state before mutation and acknowledgement; if durable state is already ahead of an exact event, the listener treats that difference as a missed invalidation, full-clears and records the durable offset. Generation loss/regression clears local values but keeps critical readiness failed until the previous epoch is restored. Source evidence covers two serving replicas, exact/wildcard values, durable-ahead recovery, deterministic local lag, missed PubSub reconciliation and Redis state-loss/restoration with the next `N+1` event | Source wiring hardened; compiled/live Redis execution pending |
| Rate-limit memory backend | In-process request counters | Entry-count Moka; 100,000 entries; fixed-size counter value; trusted identity dimensions are bounded IP/UUID fields | Time-to-idle equals the configured rate-limit window | Process-local by design. Distributed deployments requiring one global budget must select the Redis backend; Redis identities are SHA-256 hashed. The periodic Moka maintenance worker exits when its task-owned `Arc` is the final limiter reference, so runtime teardown does not retain an orphan cache | Hardened for declared modes |
| Rate-limit Redis backend | Redis counter script | Redis keys contain configured namespace plus SHA-256 identity; operation timeout is 2 s | Redis expiry equals the bounded rate-limit window | Atomic `INCR`/`EXPIRE` script contract; backend failure is fail-closed with HTTP 503, not local fallback | Hardened |
| Marketplace catalog-list cache | Registry HTTP API plus local manifest provider | Byte-weighted Moka; default 16 MiB; SHA-256 length-delimited key; response stream limited before JSON allocation; global fetch semaphore | 60 s configurable TTL | Process-local cache is acceptable for marketplace discovery. Misses are single-flight and registry failure falls back to the local catalog | Hardened |
| Marketplace module-detail cache | Registry detail endpoint with catalog fallback | Separate byte-weighted Moka; default 4 MiB; bounded SHA-256 slug key | Positive TTL follows catalog TTL; missing detail uses independently configurable 5 s default negative TTL | Process-local discovery cache; single-flight detail fetch and bounded fallback to catalog list | Hardened |
| RBAC permission snapshot cache | RBAC relation tables and resolver | Byte-weighted Moka; 16 MiB; typed `(tenant_id, user_id)` key; 64 bounded epoch stripes plus global epoch | 60 s TTL; conditional publication retries a superseded DB read and fails closed after bounded attempts | Database-backed durable generation is reserved in the mutation transaction; local/Redis PubSub is a fast path; watchdog and reconciliation clear missed generations across replicas | Hardened; owned by `rustok-rbac` plan |
| Flex field-definition cache | User/product/order/topic field-definition tables; durable source is `flex_field_definition_cache_generation` | Byte-weighted Moka; 16 MiB; `(tenant_id, entity_type)` key; variable JSON fields included in weight | 30 s TTL | Local EventBus invalidates exact entries and full-clears on lag. Transaction-local triggers on all four owner tables advance one singleton generation for insert/update/delete, including reorder and soft delete. The reconciler separates task liveness from durable readiness, full-clears before recording startup/advance, and clears plus fails closed on database error/regression. Source evidence covers the complete SQLite owner matrix, PostgreSQL rollback/concurrency/replay with an independent reader, and two replica caches through startup, advancement, database outage/recovery and regression | Source wiring hardened; compiled PostgreSQL and server execution pending |
| SEO redirect cache | `seo_redirect` database rows; durable recovery source is the `source_kind=redirect` delivery log | Byte-weighted Moka; 8 MiB; UUID tenant key; redirect string payloads included in weight. Persisted cursor query is indexed by `(source_kind, created_at, id)` and consumed in batches of 256 | 30 s TTL; reconciliation polls every 5 s | Redirect transition and delivery row are written in the same transaction, then the committing replica invalidates after commit. Every serving runtime reads count and high-water cursor before startup full-clear, invalidates exact tenants for later rows and compares processed rows with the independent count. Mismatch or query failure clears the namespace and reseeds or restarts fail-closed. Two-replica source evidence covers startup, exact delivery, multi-page catch-up, out-of-order cursor gaps, database outage/recovery and terminal isolation of one worker while the other remains ready | Source wiring hardened; compiled multi-replica execution pending |

## Compatibility paths that are not production cache owners

- `MarketplaceCatalogService::evolutionary_defaults()` and the legacy
  `RegistryMarketplaceProvider` retain the old count-only registry cache for compatibility/tests.
  Production bootstrap is guarded to construct `HardenedRegistryMarketplaceProvider` directly.
- The historical `rustok_core::cache::{InMemoryCacheBackend, FallbackCacheBackend}` module path is
  retained for compatibility. The root `rustok_core` exports route active callers through the
  atomic Moka entry-compute backend, and architecture guards reject restoring the historical root
  export or legacy fallback factory in production wiring.

## Remaining work

### Verification and tuning

- Execute the wired channel reader, two-runtime, listener-lag/value, resolved-value, PostgreSQL,
  live Redis and self-hosted Redis restart jobs on one revision.
- Execute the wired tenant-locale exact/wildcard, durable-ahead, local lag, missed-publication and
  Redis state-loss/restoration scenarios on that revision.
- Execute the wired Flex SQLite owner matrix, PostgreSQL transaction/concurrency/replay and
  two-replica startup/outage/regression scenarios on that revision.
- Execute the wired SEO seed-before-clear, exact tenant, multi-page, count/cursor-gap, database
  outage/recovery and terminal-worker scenarios across two serving replicas.
- Measure observed encoded/estimated entry sizes before changing byte budgets.
- Exercise marketplace hot-key contention, channel token rollover and RBAC epoch rotation.
- Run isolated Redis latency/repeated-restart/CAS evidence before promoting cache hardening to live
  verified.

## Maintenance rule

Update this inventory whenever an active cache is added, removed, changes capacity unit, changes
its key/value schema, or adopts a different cross-replica invalidation contract. Do not list
presentation-only memoization or test-only caches here.
