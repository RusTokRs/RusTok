# rustok-content

## Purpose

`rustok-content` provides shared content helpers and a port-based cross-domain orchestration core for RusToK.

## Responsibilities

- Provide `ContentModule` metadata for the runtime registry.
- Own shared content entities, shared migrations, and orchestration state.
- Provide shared locale, slug, and rich-text helpers used by domain modules.
- Own orchestration state, idempotency, audit records, and canonical URL/alias mappings for cross-domain flows.
- Expose a port-based `ContentOrchestrationService` that delegates domain work through `ContentOrchestrationBridge`.
- Publish only orchestration-facing RBAC for `forum_topics:*` and `blog_posts:*`.

## Interactions

- Depends on `rustok-core` for permissions, events, and `SecurityContext`.
- Depends on `rustok-api` for shared tenant/auth/request and GraphQL helper contracts.
- No longer exposes product GraphQL, REST, admin, or storefront entry points.
- Used as a shared helper dependency by `rustok-blog`, `rustok-forum`,
  `rustok-comments`, and `rustok-pages`.
- Declares permissions via `rustok-core::Permission`.
- `ContentOrchestrationService` enforces orchestration permissions from
  `AuthContext.permissions`, persists idempotency/audit state, and publishes
  orchestration events. Runtime adapters for domain conversions live outside the
  shared helper layer and implement `ContentOrchestrationBridge`.
- `apps/server` is the current runtime host for the bridge implementation and
  exposes the live GraphQL mutations for `topic ↔ post`, `split_topic`, and
  `merge_topics`.

- Conversion flows persist typed redirect/canonical state in
  `content_canonical_urls` and `content_url_aliases` and publish
  `CanonicalUrlChanged` / `UrlAliasPurged` through the outbox contract.

## Entry points

- `ContentModule`
- `ContentOrchestrationService`
- `ContentOrchestrationBridge`
- `CategoryService`
- content DTO and entity re-exports

`NodeService` remains available only under `rustok-content::services` as a
shared-node helper surface. It is intentionally no longer part of the top-level
crate entry points.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
