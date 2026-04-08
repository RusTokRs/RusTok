# rustok-cache

## Purpose

`rustok-cache` centralizes cache backend lifecycle for RusToK, including Redis-backed and
in-memory cache implementations.

## Responsibilities

- Provide `CacheModule` metadata for the runtime registry.
- Own `CacheService` and backend selection logic.
- Expose cache health information to server runtime wiring.

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

See also [docs/README.md](docs/README.md).
