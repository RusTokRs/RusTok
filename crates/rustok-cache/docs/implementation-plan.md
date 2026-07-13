# Implementation plan for `rustok-cache`

## Current state

`rustok-cache` is the capability-only core owner of backend selection, in-memory fallback,
Redis integration, invalidation, typed values and anti-stampede loading. The host consumes
the cache contract; it must not distribute backend-specific wiring or invalidation policy
across unrelated modules.

The source contract now includes:

- Redis millisecond TTL with zero-TTL invalidation and positive sub-millisecond rounding;
- bounded Redis connection and command latency in core and service-level paths;
- circuit-breaker accounting for command timeouts;
- bounded degraded-write markers that distinguish outage writes from ordinary Redis misses;
- shared invalidation error propagation instead of silent success;
- backend-scoped, cancellation-safe `load_or_fill` gates;
- count-limited and byte-weighted Moka backends exposed through `CacheService`;
- deterministic TTL jitter and leader loader deadlines;
- tenant-aware/versioned bounded key construction;
- typed Postcard envelopes with schema/source/freshness metadata and decode size limits;
- typed loading that invalidates corrupted, incompatible and hard-expired entries;
- bounded stale-while-revalidate coordination with per-key deduplication;
- explicit typed negative-cache policy with short independent TTLs;
- shared namespace generation counters for scan-free recovery;
- token-owned Redis leases with compare-and-release/extend scripts;
- versioned invalidation payloads and generation-gap detection;
- fail-safe full invalidation of field-definition cache state after event-consumer lag;
- byte-weighted capacity in the active field-definition schema cache;
- synchronized crate and architecture documentation.

Redis pub/sub remains a best-effort at-most-once fast path. The capability can detect gaps
when callers supply a durable monotonic generation and can rotate a shared namespace
generation, but the transactional outbox/stream source remains domain/platform work.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This capability has no module-owned UI or published FBA provider contract.

## Completed source phases

1. **TTL correctness**
   - Redis uses `PX` millisecond expiration;
   - zero TTL invalidates immediately;
   - positive sub-millisecond TTL rounds up to 1 ms.

2. **Fallback consistency**
   - writes made during Redis failure are tracked by a same-TTL marker;
   - ordinary Redis misses do not return stale mirrored local data;
   - shared invalidation failures are returned to callers;
   - fallback statistics include local state.

3. **Invalidation gap safety**
   - field-definition event lag clears the complete local schema cache;
   - versioned invalidation payloads carry caller-owned durable generations;
   - an unseeded first event and detected gaps require recovery;
   - the tracker distinguishes in-order, duplicate, stale and missing generations.

4. **Anti-stampede lifecycle**
   - gate identity includes backend instance and key;
   - different backends do not block on equal keys;
   - RAII cleanup covers success, errors and task cancellation;
   - policy loading supports bounded leader deadlines.

5. **Resource bounds**
   - Redis backend and cache-service operations have deadlines;
   - local caches support byte-weighted capacity;
   - weighted factories remain centralized in `CacheService`;
   - envelope encoding/decoding and cache keys have explicit maximum sizes;
   - the active Flex field-definition cache is byte-weighted.

6. **Key and value compatibility**
   - `CacheKeyBuilder` includes service, environment, tenant/global scope, domain, schema and
     resource components;
   - unsafe or overlong identities are SHA-256 hashed;
   - `CacheEnvelope<T>` rejects unsupported format/schema versions and invalid freshness
     metadata;
   - typed loading removes corrupted, mismatched and hard-expired entries before reload.

7. **Avalanche and hot-key controls**
   - deterministic per-key TTL jitter is available through `CacheLoadPolicy`;
   - token-safe Redis leases use `SET NX PX` and ownership-checking Lua scripts;
   - `CacheRefreshCoordinator` returns stale values while one bounded process-local refresh
     runs and records refresh saturation/failure metrics.

8. **Negative caching**
   - positive and negative values can use separate schema namespaces/backends;
   - negative policy requires a positive independent TTL and size limit;
   - only explicitly classified stable negatives are stored;
   - corrupted, mismatched and expired negatives are invalidated.

9. **Generation recovery primitive**
   - Redis `INCR` rotates a namespace without `SCAN` or wildcard deletion;
   - generation reads expose shared/local-fallback source;
   - a failed shared bump is returned as an error rather than acknowledged locally;
   - generation statistics expose read/bump failures and fallback reads.

## Open results

1. **Run compiled cache contract coverage.** Execute the targeted unit suite for backend
   selection, count/weighted capacity, fallback, TTL boundaries, Redis timeout helpers,
   key/envelope policy, typed loading, refresh, negative caching, leases, generation tracking,
   invalidation validation, metrics and health semantics.

   **Depends on:** GitHub Actions completion or another compilation-capable environment.

   **Done when:**

   ```bash
   cargo xtask module validate cache
   cargo xtask module test cache
   cargo test -p rustok-cache --lib
   ```

   pass without skipped relevant coverage.

2. **Collect real Redis evidence.** Run publisher/subscription, backend TTL/timeout, generation
   and lease ownership scenarios against an isolated Redis service.

   **Depends on:** `RUSTOK_CACHE_REAL_REDIS_URL`, isolated Redis and preferably a fault proxy.

   **Done when:** validated publish/subscription, PX expiry, reconnect, delayed-operation,
   generation bump/read, lease contention/expiry and compare-and-release scenarios pass with
   observable metrics.

3. **Eliminate duplicate Redis backend construction.** Build all Redis backends from the
   `CacheService`-owned client rather than reopening a client from the URL for each namespace.

   **Done when:** count and weighted factories share the central client constructor and no
   backend factory needs the raw URL after service initialization.

4. **Adopt key/envelope/policy APIs in host caches.** Migrate tenant, RBAC and other critical
   callers from hand-built keys and unversioned JSON to `CacheKeyBuilder`, `CacheEnvelope`,
   `CacheLoadPolicy`, negative policy and namespace generations.

   **Done when:** no correctness-sensitive host cache silently accepts incompatible payloads,
   synchronized fixed TTLs or a process-local-only invalidation result.

5. **Connect durable recoverable invalidation.** Supply `VersionedCacheInvalidation`
   generations from transactional outbox/event offsets, seed consumers from persisted offsets
   and execute domain recovery actions on `UnverifiedFirst`/`Gap`.

   **Done when:** a disconnected instance can clear/rebuild, rotate generation and resume from
   a durable offset rather than relying only on TTL.

6. **Operational proof.** Add load and chaos gates for synchronized expiry, oversized
   payloads, Redis latency/restart, generation read/bump failure, refresh saturation, lease
   expiry and invalidation listener lag.

## Verification

```bash
cargo xtask module validate cache
cargo xtask module test cache
cargo test -p rustok-cache --lib
RUSTOK_CACHE_REAL_REDIS_URL=redis://... \
  cargo test -p rustok-cache \
  real_redis_publish_and_subscription_share_validated_channel_contract \
  -- --ignored --nocapture
```

## Change rules

1. Keep backend wiring, invalidation and fallback policy in this module.
2. Update the crate README, local docs and `rustok-module.toml` with a cache contract change.
3. Update `docs/modules/registry.md` if module ownership or capability status changes.
4. Do not claim cache hardening complete until compiled and live Redis evidence is recorded.
5. Prefer correctness-preserving misses over serving unversioned stale values.
