# rustok-cache

## Purpose

`rustok-cache` centralizes cache backend lifecycle for RusToK, including Redis-backed and
in-memory cache implementations.

## Responsibilities

- Provide `CacheModule` metadata for the runtime registry.
- Own `CacheService`, `CacheBackendOptions`, and backend selection logic.
- Expose cache health information, service-level Prometheus gauges, invalidation publish/rejection counters, and lightweight hit/miss/invalidation statistics to server runtime wiring.
- Provide `CacheService::load_or_fill` as the generic per-key loader/coalescing contract for anti-stampede protection.
- Provide `CacheService::publish_invalidation` / `CacheInvalidationService` for validated namespaced cache invalidation publishing with Redis pub/sub, local fan-out, channel-scoped local subscriptions, and a reusable Redis pub/sub subscription adapter for host/runtime listeners.
- Keep Redis circuit breaker configuration centralized at the cache factory boundary.

## Interactions

- Depends on `rustok-core` for module contracts.
- Used by `apps/server` to build cache backends for tenant, RBAC, and other runtime caches.
- Does not publish its own RBAC surface.
- Access to cache-backed admin operations is enforced by `apps/server` through permissions
  declared by the owning domain modules.

## Entry points

- `CacheModule`
- `CacheService`
- `CacheHealthReport`
- `CacheBackendOptions`
- `CacheLoadResult` / `CacheLoadSource`
- `CacheInvalidationMessage` / `CacheInvalidationMessageError` / `CacheInvalidationOutcome` / `CacheInvalidationStats` / `CacheInvalidationService` / `LocalCacheInvalidationSubscription`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)

## Invalidation listener contract

With the `redis-cache` feature enabled, `CacheInvalidationService::consume_subscription(channel, handler)` owns Redis pub/sub connection/subscription setup for one channel and invokes the supplied handler for each invalidation message. Invalidation messages can be pre-validated with `CacheInvalidationMessage::try_new`, and `publish_invalidation` drops empty channel/key messages before local or Redis fan-out. Host runtimes keep their domain-specific retry loop, health status, and telemetry around this adapter; `CacheInvalidationService::stats()` and `CacheService::prometheus_metrics()` expose local publish, Redis publish success/failure, and validation rejection counters; without Redis, `subscribe_local()` remains the full single-instance/test fan-out contract and `subscribe_local_channel(channel)` mirrors Redis channel filtering for namespace-specific listeners.
