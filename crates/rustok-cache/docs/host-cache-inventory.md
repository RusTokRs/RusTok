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
| Channel resolution cache | Channel database state through `ChannelResolver`; durable source is `channel_resolution_invalidation_state` | Byte-weighted Moka; 16 MiB; typed key containing tenant, bounded request facts and a monotonic tenant token | 60 s positive TTL; 10 s negative TTL | Mutations to all six channel-owned tables increment one database generation through transaction-local triggers. Successful REST/native writes clear the local tenant token and publish `channel.resolution.generation.v1` as a fast path. Supervised local/Redis/reconcile workers compare the persisted generation every 5 s and perform a safe namespace-wide local clear when delivery was missed, startup is unverified, or generation regresses. The per-tenant token registry remains bounded at 16,384 tenants; capacity rollover clears all entries and allocator exhaustion disables caching fail-safe | Source hardened; compiled and multi-replica recovery evidence pending |
| Tenant locale cache | `tenant_locales` database rows | Byte-weighted Moka; 8 MiB; UUID tenant key; atomic process-local registration | 60 s TTL; empty vectors are ordinary short-lived results rather than a separate negative schema | Shares `tenant.cache.generation.v1`: in-order UUID records invalidate one tenant, `*` invalidates the namespace, and unverified/gapped/lagged or reconciled advancement clears all entries before acknowledgement. Local/Redis/reconcile tasks are context-owned and required Redis delivery is a critical readiness dependency | Source hardened; multi-replica execution evidence pending |
| Rate-limit memory backend | In-process request counters | Entry-count Moka; 100,000 entries; fixed-size counter value; trusted identity dimensions are bounded IP/UUID fields | Time-to-idle equals the configured rate-limit window | Process-local by design. Distributed deployments requiring one global budget must select the Redis backend; Redis identities are SHA-256 hashed. The periodic Moka maintenance worker exits when its task-owned `Arc` is the final limiter reference, so runtime teardown does not retain an orphan cache | Hardened for declared modes |
| Rate-limit Redis backend | Redis counter script | Redis keys contain configured namespace plus SHA-256 identity; operation timeout is 2 s | Redis expiry equals the bounded rate-limit window | Atomic `INCR`/`EXPIRE` script contract; backend failure is fail-closed with HTTP 503, not local fallback | Hardened |
| Marketplace catalog-list cache | Registry HTTP API plus local manifest provider | Byte-weighted Moka; default 16 MiB; SHA-256 length-delimited key; response stream limited before JSON allocation; global fetch semaphore | 60 s configurable TTL | Process-local cache is acceptable for marketplace discovery. Misses are single-flight and registry failure falls back to the local catalog | Hardened |
| Marketplace module-detail cache | Registry detail endpoint with catalog fallback | Separate byte-weighted Moka; default 4 MiB; bounded SHA-256 slug key | Positive TTL follows catalog TTL; missing detail uses independently configurable 5 s default negative TTL | Process-local discovery cache; single-flight detail fetch and bounded fallback to catalog list | Hardened |
| RBAC permission snapshot cache | RBAC relation tables and resolver | Byte-weighted Moka; 16 MiB; typed `(tenant_id, user_id)` key; 64 bounded epoch stripes plus global epoch | 60 s TTL; conditional publication retries a superseded DB read and fails closed after bounded attempts | Database-backed durable generation is reserved in the mutation transaction; local/Redis PubSub is a fast path; watchdog and reconciliation clear missed generations across replicas | Hardened; owned by `rustok-rbac` plan |
| Flex field-definition cache | User/product/order/topic field-definition tables; durable source is `flex_field_definition_cache_generation` | Byte-weighted Moka; 16 MiB; `(tenant_id, entity_type)` key; variable JSON fields included in weight | 30 s TTL | Local EventBus invalidates exact entries and full-clears on lag. Transaction-local database triggers on all four owner tables advance one singleton generation for every insert/update/delete, including reorder and soft delete. Every serving runtime reads the generation, full-clears before recording startup/advance, polls every 5 s, fails closed on database error/regression and exposes the supervised reconciler as a critical readiness dependency | Source hardened; compiled, cross-database and multi-replica recovery evidence pending |
| SEO redirect cache | `seo_redirect` database rows | Byte-weighted Moka; 8 MiB; UUID tenant key; redirect string payloads included in weight. Persisted cursor query is indexed by `(source_kind, created_at, id)` and consumed in batches of 256 | 30 s TTL; reconciliation polls every 5 s | Redirect transition and delivery row are written in the same transaction. The mutating replica invalidates after commit; every serving runtime seeds the persisted high-water cursor before a startup full clear and then invalidates exact tenants for later rows. The context-owned supervised worker is a critical readiness dependency | Source hardened; multi-replica execution evidence pending |

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

- Exercise channel mutation commit, concurrent generation advancement, publication loss, local lag,
  Redis reconnect, database outage/recovery, generation regression and periodic reconciliation
  across multiple replicas.
- Exercise exact/wildcard tenant-locale invalidation, local lag, Redis reconnect and periodic
  generation reconciliation across multiple replicas.
- Exercise Flex generation triggers on PostgreSQL and SQLite for all four owner tables, startup
  seed-before-clear, concurrent mutations, database outage/recovery, regression and critical
  readiness across multiple replicas.
- Exercise SEO cursor startup races, more-than-one-batch catch-up, database outage/recovery and
  terminal-worker readiness across multiple serving replicas.
- Measure observed encoded/estimated entry sizes before changing byte budgets.
- Exercise marketplace hot-key contention, channel token rollover and RBAC epoch rotation.
- Run isolated Redis outage/reconnect/CAS/invalidations before promoting cache hardening to live
  verified.

## Maintenance rule

Update this inventory whenever an active cache is added, removed, changes capacity unit, changes
its key/value schema, or adopts a different cross-replica invalidation contract. Do not list
presentation-only memoization or test-only caches here.
