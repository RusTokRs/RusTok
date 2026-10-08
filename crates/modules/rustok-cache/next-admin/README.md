# @rustok/cache-admin

Next.js admin UI package for the `rustok-cache` module.

## Responsibilities

- Exposes the cache management root page and settings form for `apps/next-admin`.
- Communicates with RusToK GraphQL API via typed queries (`cacheHealth`, `platformSettings`) and mutations (`updatePlatformSettings`).
- Provides cache diagnostics cards and dynamic configuration controls for In-Memory, Redis, and Hybrid caching modes.
- Provides module navigation metadata for the host sidebar.
