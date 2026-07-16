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
| Channel resolution cache | Channel database state through `ChannelResolver` | Byte-weighted Moka; 16 MiB; typed key containing tenant, bounded request facts and a monotonic tenant token | 60 s positive TTL; 10 s negative TTL | Atomic process-local registration. Per-tenant generation registry is bounded at 16,384 tenants; capacity rollover clears all entries and allocator exhaustion disables caching fail-safe. Cross-replica invalidation is not currently durable | Source hardened; owner decision required for cross-replica stale bound |
| Tenant locale cache | `tenant_locales` database rows | Byte-weighted Moka; 8 MiB; UUID tenant key; atomic process-local registration | 60 s TTL; empty vectors are ordinary short-lived results rather than a separate negative schema | Explicit local invalidation only. Another replica can retain the previous locale set until TTL expiry | Source hardened; owner decision required for cross-replica stale bound |
| Rate-limit memory backend | In-process request counters | Entry-count Moka; 100,000 entries; fixed-size counter value; trusted identity dimensions are bounded IP/UUID fields | Time-to-idle equals the configured rate-limit window | Process-local by design. Distributed deployments requiring one global budget must select the Redis backend; Redis identities are SHA-256 hashed. The periodic Moka maintenance worker exits when its task-owned `Arc` is the final limiter reference, so runtime teardown does not retain an orphan cache | Hardened for declared modes |
| Rate-limit Redis backend | Redis counter script | Redis keys contain configured namespace plus SHA-256 identity; operation timeout is 2 s | Redis expiry equals the bounded rate-limit window | Atomic `INCR`/`EXPIRE` Lua-style script contract; backend failure is fail-closed with HTTP 503, not local fallback | Hardened |
| Marketplace catalog-list cache | Registry HTTP API plus local manifest provider | Byte-weighted Moka; default 16 MiB; SHA-256 length-delimited key; response stream limited before JSON allocation; global fetch semaphore | 60 s configurable TTL | Process-local cache is acceptable for marketplace discovery. Misses are single-flight and registry failure falls back to the local catalog | Hardened |
| Marketplace module-detail cache | Registry detail endpoint with catalog fallback | Separate byte-weighted Moka; default 4 MiB; bounded SHA-256 slug key | Positive TTL follows catalog TTL; missing detail uses independently configurable 5 s default negative TTL | Process-local discovery cache; single-flight detail fetch and bounded fallback to catalog list | Hardened |
| RBAC permission snapshot cache | RBAC relation tables and resolver | Byte-weighted Moka; 16 MiB; typed `(tenant_id, user_id)` key; 64 bounded epoch stripes plus global epoch | 60 s TTL; conditional publication retries a superseded DB read and fails closed after bounded attempts | Database-backed durable generation is reserved in the mutation transaction; local/Redis PubSub is a fast path; watchdog and reconciliation clear missed generations across replicas | Hardened; owned by `rustok-rbac` plan |
| Flex field-definition cache | Flex field-definition database state | Byte-weighted Moka; 16 MiB; `(tenant_id, entity_type)` key; variable JSON fields included in weight | 30 s TTL | Event consumer invalidates exact entries; lag triggers full clear. Cache and consumer share one restartable abort-on-drop runtime. Transport is process-local unless the event owner supplies a durable source | Source hardened; durable cross-replica recovery remains owner work |
| SEO redirect cache | `seo_redirect` database rows | Byte-weighted Moka; 8 MiB; UUID tenant key; redirect string payloads included in weight | 30 s TTL | Mutating replica invalidates after transaction commit. Transactional domain events are emitted, but no cache-generation consumer currently invalidates other replicas; stale redirects are therefore bounded only by TTL | Source hardened; durable owner invalidation candidate |

## Compatibility paths that are not production cache owners

- `MarketplaceCatalogService::evolutionary_defaults()` and the legacy
  `RegistryMarketplaceProvider` retain the old count-only registry cache for compatibility/tests.
  Production bootstrap is guarded to construct `HardenedRegistryMarketplaceProvider` directly.
- The legacy `rustok-core::FallbackCacheBackend` is not the production fallback factory. Active
  factories use the degradation-aware `rustok-cache` implementation and architecture guards reject
  restoring the legacy path.

## Remaining migrations

### Owner decisions required

- Channel: document whether a 60-second cross-replica stale bound is acceptable. Otherwise add an
  owner generation/outbox consumer and clear the affected tenant generation on missed events.
- Locale: document whether a 60-second stale bound is acceptable for locale enablement/default
  changes. Otherwise publish a tenant-locale generation in the committing transaction.
- Field definitions: connect the existing full-clear recovery to a durable event offset or shared
  generation when multiple replicas consume independent local buses.
- SEO redirects: consume the already transactional redirect event on every replica or reserve a
  durable SEO redirect generation in the same transaction, then invalidate the tenant cache before
  acknowledging recovery.

### Verification and tuning

- Measure observed encoded/estimated entry sizes before changing byte budgets.
- Exercise marketplace hot-key contention, channel generation rollover and RBAC epoch rotation.
- Run isolated Redis outage/reconnect/CAS/invalidations before promoting cache hardening to live
  verified.

## Maintenance rule

Update this inventory whenever an active cache is added, removed, changes capacity unit, changes
its key/value schema, or adopts a different cross-replica invalidation contract. Do not list
presentation-only memoization or test-only caches here.
