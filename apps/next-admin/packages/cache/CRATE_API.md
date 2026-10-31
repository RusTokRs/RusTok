# apps/next-admin/packages/cache / CRATE_API

## Purpose

`@rustok/cache-admin` (`apps/next-admin/packages/cache`) is the Next.js administration UI package for the `rustok-cache` platform module. It provides operational visibility into cache engine health and live dynamic configuration for In-Memory, Redis, and Hybrid cache tiers in the Next.js admin dashboard.

## Architecture & Responsibilities

- **Module Registration:** Registers the cache module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`cacheNavItems`) under the System section.
- **Diagnostics & Status Workspace:** Displays real-time operational status (`CacheAdminPage` / `CachePage`), showing cache backend type, Redis connection status, health indicator badges, and any active error messages.
- **Dynamic Settings Management:** Exposes interactive controls (`CacheSettingsForm`) allowing administrators to configure cache operating mode (`in-memory`, `redis`, `hybrid`) and connection parameters (host, port, database index, password, connection URL).
- **GraphQL Integration:** Directly queries `cacheHealth` and `platformSettings(category: "cache")`, and commits modifications via `updatePlatformSettings(input: { category: "cache", settings: "..." })` without intermediate server proxies.

## Public Surface

- `cacheNavItems`: Admin sidebar navigation configuration (`title: 'System'`, `items: [{ title: 'Cache', url: '/dashboard/cache' }]`, `access: { role: 'admin' }`).
- `CacheAdminPage` / `CachePage`: Server-rendered cache operations dashboard container with health diagnostics and settings integration.
- `CacheSettingsForm`: Client-side form component managing cache mode switching and Redis connection configuration with optimistic toast notifications.
- `DEFAULT_CACHE_SETTINGS`: Canonical default configuration fallback object (`mode: 'in-memory'`, `redis_host: '127.0.0.1'`, `redis_port: 6379`, `redis_db: 0`).
- `CACHE_HEALTH_QUERY`, `PLATFORM_SETTINGS_QUERY`, `UPDATE_PLATFORM_SETTINGS_MUTATION`: Exported GraphQL query and mutation documents.
- `getCacheHealth(opts)`: Fetches current cache backend status and Redis health snapshot.
- `getCacheSettings(opts)`: Fetches and safely parses persisted platform cache settings.
- `updateCacheSettings(settings, opts)`: Updates and serializes cache settings payload to the platform settings store.

## Invariants & Guarantees

1. **GraphQL Contract Hygiene:** All queries and mutations match the backend `rustok-cache` and platform schema (`cacheHealth`, `platformSettings`, `updatePlatformSettings`) with exact field selection.
2. **Safe Fallbacks:** Unparsable or missing platform settings JSON gracefully falls back to `DEFAULT_CACHE_SETTINGS` without throwing or crashing the UI.
3. **Admin Guard:** Navigation and access strictly require the `admin` role (`access: { role: 'admin' }`).
4. **Conditional Input Rendering:** Dedicated Redis connection fields (host, port, password, db, url) are conditionally rendered only when the operating mode requires Redis (`redis` or `hybrid`).
5. **Parity with Leptos Admin:** Status indicators and configuration capabilities maintain functional parity with `crates/modules/rustok-cache/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `@/modules/registry`, and `@/shared/ui/shadcn` design system components from `apps/next-admin`.
- **Backend API:** Communicates with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Storage Access:** Never accesses cache backends (Redis or in-memory stores) directly; all operations execute through the authenticated RusToK GraphQL gateway.

