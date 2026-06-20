# rustok-cache

## Purpose

`rustok-cache` centralizes cache backend lifecycle for RusToK, including Redis-backed and
in-memory cache implementations.

## Responsibilities

- Provide `CacheModule` metadata for the runtime registry.
- Own `CacheService`, `CacheBackendOptions`, and backend selection logic.
- Expose cache health information and lightweight hit/miss/invalidation statistics to server runtime wiring.
- Provide `CacheService::load_or_fill` as the generic per-key loader/coalescing contract for anti-stampede protection.
- Provide `CacheService::publish_invalidation` / `CacheInvalidationService` for namespaced cache invalidation publishing with Redis pub/sub plus local fan-out.
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
- `CacheInvalidationMessage` / `CacheInvalidationOutcome` / `CacheInvalidationService`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
