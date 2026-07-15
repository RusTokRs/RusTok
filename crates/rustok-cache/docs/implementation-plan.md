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
- strict shared-primary health reporting while request paths retain bounded local fallback;
- shared invalidation error propagation instead of silent success;
- backend-scoped, cancellation-safe and globally bounded `load_or_fill` gates;
- count-limited and byte-weighted Moka backends exposed through `CacheService`;
- default, count and weighted Redis factories reusing the client owned by `CacheService`;
- deterministic TTL jitter and leader loader deadlines;
- tenant-aware/versioned cache keys with per-identity, aggregate-input and component-count limits;
- typed Postcard envelopes with schema/source/freshness metadata and bounded pre-allocation
  serialization;
- typed loading that invalidates corrupted, incompatible and hard-expired entries;
- bounded stale-while-revalidate coordination with per-key deduplication;
- backend-level atomic compare-and-set for local and Redis stale refresh writes;
- fail-closed fallback CAS when the shared primary is unavailable;
- explicit typed negative-cache policy with short independent TTLs;
- shared namespace generation counters for scan-free recovery;
- bounded, non-evicting trusted generation snapshots for outage fallback;
- token-owned Redis leases with compare-and-release/extend scripts;
- versioned invalidation payloads and generation-gap detection;
- fail-safe full invalidation of field-definition cache state after event-consumer lag;
- byte-weighted capacity in active field-definition, tenant, channel, locale, RBAC and SEO caches;
- bounded, hashed and single-flight registry marketplace list and module-detail caches;
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

2. **Fallback consistency and health**
   - writes made during Redis failure are tracked by a same-TTL marker;
   - ordinary Redis misses do not return stale mirrored local data;
   - shared invalidation failures are returned to callers;
   - fallback statistics include local state;
   - Redis degradation remains visible to health/readiness even while bounded L1 fallback serves
     request paths.

3. **Invalidation gap safety**
   - field-definition event lag clears the complete local schema cache;
   - versioned invalidation payloads carry caller-owned durable generations;
   - an unseeded first event and detected gaps require recovery;
   - durable offsets advance only after recovery acknowledgement.

4. **Anti-stampede lifecycle**
   - gate identity includes backend instance and key;
   - different backends do not block on equal keys;
   - RAII cleanup covers success, errors and task cancellation;
   - policy loading supports bounded leader deadlines;
   - raw keys and unique in-flight gates have explicit process limits.

5. **Resource bounds**
   - Redis backend and cache-service operations have deadlines;
   - local caches support byte-weighted capacity;
   - envelope encoding is measured before output allocation and written through a bounded writer;
   - cache keys bound one identity, aggregate canonical input and dynamic component count;
   - invalidation, loader and refresh keys have explicit limits;
   - trusted generation snapshots have a non-evicting process capacity;
   - weighted factories remain centralized in `CacheService`.

6. **Key and value compatibility**
   - `CacheKeyBuilder` includes service, environment, tenant/global scope, domain, schema and
     resource components;
   - unsafe identities are SHA-256 hashed while oversized inputs are rejected before hashing;
   - `CacheEnvelope<T>` rejects unsupported format/schema versions and invalid freshness
     metadata;
   - typed loading removes corrupted, mismatched and hard-expired entries before reload.

7. **Avalanche and hot-key controls**
   - deterministic per-key TTL jitter is available through `CacheLoadPolicy`;
   - token-safe Redis leases use `SET NX PX` and ownership-checking Lua scripts;
   - `CacheRefreshCoordinator` returns stale values while one bounded process-local refresh
     runs and records refresh saturation/failure/rejection metrics.

8. **Negative caching**
   - positive and negative values can use separate schema namespaces/backends;
   - negative policy requires a positive independent TTL and size limit;
   - only explicitly classified stable negatives are stored;
   - corrupted, mismatched and expired negatives are invalidated.

9. **Generation recovery primitive**
   - Redis `INCR` rotates a namespace without `SCAN` or wildcard deletion;
   - generation reads expose shared/local-fallback source;
   - a failed shared bump is returned as an error rather than acknowledged locally;
   - shared generation regression is rejected;
   - trusted local snapshots are bounded without eviction of existing namespaces.

10. **Central Redis lifecycle**
    - `CacheService` opens the configured Redis client once;
    - default, per-call, weighted and entry-count factories reuse that client;
    - namespace backend construction no longer reopens Redis from the stored URL;
    - architecture guards reject restoration of the legacy URL-based factory.

11. **Atomic stale refresh writes**
    - `CacheBackend` exposes explicit `Applied` / `Mismatch` compare-and-set outcomes;
    - unsupported backends fail closed rather than emulating CAS with `GET` plus `SET`;
    - in-memory writes and CAS share bounded striped locks;
    - legacy and service-owned Redis backends use one binary-safe Lua compare-and-write command;
    - fallback CAS never acknowledges a process-local write when the shared primary is down;
    - all active instrumentation and weighted wrappers delegate CAS;
    - SWR publishes through CAS and treats a mismatch as an authoritative newer write.

12. **Registry marketplace host cache**
    - catalog lists use byte-weighted Moka capacity and bounded SHA-256 cache identities;
    - catalog misses are coalesced with `try_get_with` and share a global fetch semaphore;
    - response bodies are streamed with a hard byte ceiling before JSON parsing;
    - module-detail lookups use a separate byte-weighted, hashed and single-flight cache;
    - missing detail entries use a short independently configurable negative TTL;
    - the public wrapper preserves the previously verified catalog implementation byte-for-byte.

## Verification in progress

Atomic CAS source changes were merged through PR `#1713`. The focused hosted `Cache hardening`
workflow is still the source of truth for compiled and live-service evidence. The required jobs are:

- `Compiled cache contract`: format, core/cache/server compile, targeted unit and architecture
  tests, Clippy with warnings denied, and module validate/test gates;
- `Live Redis cache contract`: ignored `rustok-cache` and `rustok-core` Redis suites against an
  isolated Redis 7 service, including binary-safe CAS applied/mismatch behavior.

Do not mark the atomic CAS phase compiled/live verified until those jobs complete successfully.
The marketplace detail-cache wrapper also requires the hosted server compile and architecture test
before it can be marked operationally verified.

## Remaining work

1. **Complete host adoption.** Continue migrating correctness-sensitive caches to canonical keys,
   envelopes, explicit load/negative policy and shared generations. Tenant is the reference path;
   remaining callers should not silently accept incompatible payloads or process-local-only
   invalidation.

2. **Connect durable recoverable invalidation.** Supply `VersionedCacheInvalidation` generations
   from transactional outbox/event offsets, seed consumers from persisted offsets and execute
   domain recovery actions on `UnverifiedFirst`/`Gap`.

3. **Operational proof and tuning.** Add load and chaos gates for synchronized expiry, oversized
   payloads, Redis latency/restart, CAS contention/timeouts, generation capacity/read/bump failure,
   refresh saturation, lease expiry, invalidation listener lag and marketplace hot-slug contention.
   Tune byte budgets and TTLs from production payload distributions rather than assumptions.

4. **Local CAS expiry semantics.** Verify under stress that Moka expiration/eviction cannot revive an
   entry between comparison and replacement. If the implementation cannot prove this invariant,
   move local CAS to a backend primitive that couples value comparison and entry mutation.

## Verification commands

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo check -p rustok-cache --lib
cargo check -p rustok-server --lib
cargo test -p rustok-core cache --lib
cargo test -p rustok-cache --lib
cargo test -p rustok-server --test cache_architecture_guard --test tenant_cache_architecture_guard
cargo test -p rustok-server --test marketplace_cache_architecture_guard
cargo clippy -p rustok-core --lib -- -D warnings
cargo clippy -p rustok-cache --lib -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate cache
cargo xtask module test cache
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-cache -- --ignored --nocapture
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-core cache -- --ignored --nocapture
```

## Change rules

1. Keep backend wiring, invalidation and fallback policy in this module.
2. Update the crate README, local docs and `rustok-module.toml` with a cache contract change.
3. Update `docs/modules/registry.md` if module ownership or capability status changes.
4. Do not claim cache hardening complete until compiled and live Redis evidence is recorded.
5. Prefer correctness-preserving misses over serving unversioned stale values.
