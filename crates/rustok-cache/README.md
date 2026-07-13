# rustok-cache

## Purpose

`rustok-cache` centralizes cache backend lifecycle and failure policy for RusToK. It owns
Redis-backed and in-memory implementations, bounded degraded-mode fallback, anti-stampede
loading, invalidation transport and cache operability signals.

## Responsibilities

- Provide `CacheModule` metadata for the runtime registry.
- Own `CacheService`, `CacheBackendOptions`, backend selection and Redis lifecycle.
- Expose count-limited and byte-weighted in-memory backend factories.
- Bound Redis connection and command latency so cache failures cannot indefinitely occupy
  request tasks.
- Keep Redis circuit-breaker configuration centralized at the cache factory boundary.
- Provide `CacheService::load_or_fill` as the generic cancellation-safe, backend-scoped
  per-key loader/coalescing contract for anti-stampede protection.
- Provide bounded in-memory fallback during Redis degradation without treating an ordinary
  Redis miss as permission to return an unrelated stale local value.
- Surface failed shared invalidation instead of silently acknowledging a potentially stale
  Redis entry.
- Provide validated namespaced invalidation publishing with Redis pub/sub, local fan-out,
  channel-scoped local subscriptions and a reusable Redis subscription adapter.
- Expose cache health, Prometheus gauges, invalidation publish/rejection counters and
  lightweight hit/miss/invalidation statistics.

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
Service-level health checks, pub/sub setup/subscription and invalidation PUBLISH are bounded
as well. Timeout failures participate in the circuit-breaker failure path.

Redis TTL uses millisecond precision. Positive sub-millisecond durations are rounded up to
one millisecond; a zero TTL performs immediate invalidation instead of issuing an invalid
`PX 0`/`EX 0` command.

## Anti-stampede contract

`CacheService::load_or_fill` coalesces only callers sharing both the same backend instance
and cache key. Identical keys belonging to different namespaces/backends do not block one
another. Gate leases are released on success, loader/storage error and future cancellation,
so cancelled tasks cannot leak in-flight keys indefinitely.

## Invalidation listener contract

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

- `CacheModule`
- `CacheService`
- `CacheService::backend` / `backend_weighted`
- `CacheService::memory_backend` / `memory_backend_weighted`
- `CacheHealthReport`
- `CacheBackendOptions`
- `CacheLoadResult` / `CacheLoadSource`
- `CacheInvalidationMessage` / `CacheInvalidationMessageError`
- `CacheInvalidationOutcome` / `CacheInvalidationStats`
- `CacheInvalidationService` / `LocalCacheInvalidationSubscription`

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

## Docs

- [Module docs](./docs/README.md)
- [Caching architecture](./docs/CACHING_ARCHITECTURE.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
