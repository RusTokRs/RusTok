# rustok-cache

## Purpose

`rustok-cache` centralizes cache backend lifecycle and failure policy for RusToK. It owns
Redis-backed and in-memory implementations, bounded degraded-mode fallback, anti-stampede
loading, typed/versioned values, invalidation recovery and cache operability signals.

## Responsibilities

- Provide `CacheModule` metadata for the runtime registry.
- Own `CacheService`, `CacheBackendOptions`, backend selection and Redis lifecycle.
- Expose count-limited and byte-weighted in-memory backend factories.
- Bound Redis connection and command latency so cache failures cannot indefinitely occupy
  request tasks.
- Keep Redis circuit-breaker configuration centralized at the cache factory boundary.
- Provide cancellation-safe, backend-scoped request coalescing and optional deterministic
  TTL jitter/loader deadlines.
- Provide canonical tenant-aware/versioned cache keys and bounded typed value envelopes.
- Provide bounded stale-while-revalidate coordination and explicit negative caching.
- Provide shared namespace generations and token-safe distributed leases.
- Provide bounded in-memory fallback during Redis degradation without treating an ordinary
  Redis miss as permission to return an unrelated stale local value.
- Surface failed shared invalidation instead of silently acknowledging a potentially stale
  Redis entry.
- Provide validated namespaced invalidation publishing, versioned invalidation payloads and
  generation-gap detection.
- Expose cache health, refresh/generation statistics, invalidation counters and lightweight
  hit/miss/invalidation statistics.

## Backend factories and capacity

`CacheService::backend(prefix, ttl, max_capacity)` and
`CacheService::memory_backend(ttl, max_capacity)` preserve the historical entry-count
capacity contract.

For variable-size serialized documents, use
`CacheService::backend_weighted(prefix, ttl, max_weight_bytes)` or
`CacheService::memory_backend_weighted(ttl, max_weight_bytes)`. Weighted capacity includes
key bytes, payload bytes and per-entry metadata, preventing a small number of oversized
values from bypassing an entry-count limit.

All factories preserve the same `CacheBackend` and instrumentation contract. With Redis
configured, `backend*` returns Redis primary plus the corresponding bounded in-memory
fallback; otherwise it returns the in-memory backend directly.

## Canonical keys and typed envelopes

`CacheKeyBuilder` creates keys in the shape
`service:environment:tenant-or-global:domain:schema-version:resource:identity`. Fixed
namespace components are validated. Dynamic identities remain readable only when short and
safe; otherwise they become a SHA-256 digest. Complete keys are capped at 512 bytes.

`CacheEnvelope<T>` uses a versioned Postcard wire format and records:

- cache envelope format version;
- domain schema version;
- generation timestamp;
- optional source revision;
- optional soft and hard expiration boundaries;
- typed payload.

Encoding and decoding enforce a size limit before returning/allocating unbounded cache data.
Schema mismatch, unsupported format, invalid expiration ordering and oversized payloads are
explicit errors.

`CacheService::load_enveloped_or_fill` combines the envelope with coalesced loading. It
invalidates corrupted, schema-incompatible and hard-expired values before reload. A value
between soft and hard expiry is returned as `Stale` rather than silently treated as fresh.

## Fallback and consistency semantics

Writes are retained in the local fallback before the Redis write is attempted. If the Redis
write fails, a bounded marker with the same TTL records that the local value is authoritative
for degraded operation. After Redis reconnects:

- a Redis hit wins and clears the degraded marker;
- a Redis miss may use local data only while a matching degraded-write marker is alive;
- an ordinary Redis miss never returns a previously mirrored local value;
- a successful shared write clears the marker;
- invalidation always clears local state and returns an error when Redis deletion fails.

The last rule is deliberate: callers must not report successful mutation/invalidation while
a stale shared entry may still exist on another process.

## Redis timing and TTL guarantees

Redis backend construction, GET, SET, DEL and PING are bounded by an operation timeout.
Service-level health checks, pub/sub setup/subscription, generation operations, distributed
lease operations and invalidation PUBLISH are bounded as well.

Redis TTL uses millisecond precision. Positive sub-millisecond durations are rounded up to
one millisecond; a zero TTL performs immediate invalidation instead of issuing an invalid
`PX 0`/`EX 0` command.

## Anti-stampede, avalanche and stale refresh

`CacheService::load_or_fill` coalesces only callers sharing both the same backend instance
and cache key. Identical keys belonging to different namespaces/backends do not block one
another. Gate leases are released on success, loader/storage error and future cancellation,
so cancelled tasks cannot leak in-flight keys indefinitely.

`CacheService::load_or_fill_with_policy` adds:

- deterministic TTL jitter for `(namespace, key)`, bounded to ±50%;
- an optional deadline around the leader's source-of-truth loader.

The jitter is stable rather than random, so retries and tests produce the same expiry while
large namespaces avoid synchronized expiration.

`CacheRefreshCoordinator` implements bounded stale-while-revalidate:

- stale values are returned until hard expiry;
- refresh identity is `(backend, key)`;
- duplicate refreshes are coalesced;
- a semaphore caps total process-local refresh concurrency;
- failed refreshes leave the stale value untouched;
- metrics expose started, completed, failed, deduplicated and saturated work.

For loaders whose cross-instance amplification justifies a distributed lock,
`try_acquire_distributed_lease` uses Redis `SET NX PX` with a UUID ownership token. Lease
extension and release use compare-and-PEXPIRE/delete Lua scripts, preventing one process
from modifying a lock that expired and was acquired by another owner.

## Negative caching

`NegativeCachePolicy` requires a non-zero schema version, a bounded positive TTL and an
encoded-size limit. Only explicitly classified stable domain negatives are stored through
`store_negative`; transport failures, dependency errors and timeouts cannot be implicitly
converted into cached not-found responses.

Negative entries use typed versioned envelopes. Corrupted, schema-incompatible and
hard-expired entries are invalidated and treated as misses. Deterministic TTL jitter can be
used for high-cardinality negative namespaces.

## Namespace generations and invalidation recovery

`CacheNamespaceGenerationStore` exposes a shared Redis generation counter and a local
fallback snapshot. Cache keys can include `generation.key_component()`. Bumping the counter
makes all previous-generation keys unreachable without `SCAN`, wildcard deletion or a large
invalidation fan-out.

When Redis is configured, a failed bump is returned as an error: a local-only increment is
never acknowledged as cross-instance invalidation. Reads may fall back to the last locally
observed generation during Redis degradation and expose that source explicitly.

With the `redis-cache` feature enabled,
`CacheInvalidationService::consume_subscription(channel, handler)` owns bounded Redis
pub/sub connection/subscription setup for one channel and invokes the supplied handler for
each valid invalidation message. `CacheInvalidationMessage::try_new` validates messages
before publish, and `publish_invalidation` drops empty channel/key messages before local or
Redis fan-out.

Redis pub/sub remains an at-most-once, best-effort transport: messages published while a
subscriber is disconnected are not replayed. Domain listeners must retain retry/health
telemetry and use fail-safe recovery when they detect a gap. For example, the field-definition
cache clears all entries when its event receiver reports lag.

For domains with a durable outbox/event sequence, `VersionedCacheInvalidation` carries that
monotonic generation and `CacheInvalidationGapTracker` classifies unverified-first, in-order,
duplicate, stale and gap observations. An unseeded first event requires recovery; `seed()`
accepts a persisted consumer offset. A detected gap also requires namespace recovery before
later entries are trusted.

`CacheInvalidationService::stats()` and `CacheService::prometheus_metrics()` expose local
publish, Redis publish success/failure and validation rejection counters. Without Redis,
`subscribe_local()` remains the single-instance/test fan-out contract and
`subscribe_local_channel(channel)` provides namespace filtering.

## Interactions

- Depends on `rustok-core` for cache and module contracts.
- Used by `apps/server` to build tenant, RBAC and other runtime caches.
- Does not publish its own RBAC or UI surface.
- Access to cache-backed admin operations is enforced by the host through permissions
  declared by owning domain modules.

## Entry points

- `CacheModule` / `CacheService`
- `CacheService::backend` / `backend_weighted`
- `CacheService::memory_backend` / `memory_backend_weighted`
- `CacheService::load_or_fill` / `load_or_fill_with_policy`
- `CacheService::load_enveloped_or_fill`
- `CacheService::load_enveloped_stale_while_revalidate`
- `CacheKeyBuilder`
- `CacheEnvelope<T>` / `TypedCacheLoadResult<T>`
- `CacheLoadPolicy` / `CacheTtlPolicy`
- `CacheRefreshCoordinator` / `StaleWhileRevalidateResult<T>`
- `NegativeCachePolicy` / `NegativeCacheEntry<T>`
- `CacheNamespaceGenerationStore` / `CacheNamespaceGeneration`
- `CacheLeaseOptions` / `CacheLeaseOutcome` / `DistributedCacheLease`
- `CacheInvalidationMessage` / `VersionedCacheInvalidation`
- `CacheInvalidationGapTracker` / `CacheInvalidationObservation`
- `CacheInvalidationService` / `LocalCacheInvalidationSubscription`
- `CacheHealthReport` / `CacheBackendOptions`

## Verification

Source changes are not a substitute for compiled and live-service evidence. Run:

```bash
cargo xtask module validate cache
cargo xtask module test cache
cargo test -p rustok-cache --lib
```

For the ignored Redis integration gate:

```bash
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379 \
  cargo test -p rustok-cache \
  real_redis_publish_and_subscription_share_validated_channel_contract \
  -- --ignored --nocapture
```

The repository-wide CI workflow runs formatting, clippy, workspace checks, MSRV checks and
nextest on every push. Live Redis lease, generation and fault-injection tests still require an
isolated service and explicit execution.

## Docs

- [Module docs](./docs/README.md)
- [Caching architecture](./docs/CACHING_ARCHITECTURE.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
