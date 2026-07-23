# rustok-content

## Purpose

`rustok-content` provides shared content helpers and a port-based cross-domain orchestration core for RusToK.

The target richtext boundary keeps neutral document/read-projection types in
`rustok-api::richtext` and executable profiles, validation, safe HTML rendering,
and plain-text extraction in `rustok-content::richtext`. Blog, Forum, Comments,
and future consumers continue to own their localized rows and revisions. See
the [central implementation plan](../../docs/modules/rich-text-implementation-plan.md).

## Responsibilities

- Provide `ContentModule` metadata for the runtime registry.
- Own shared content entities, shared migrations, and orchestration state.
- Provide shared locale and slug helpers and the target executable richtext
  policy used by domain modules.
- Provide the first executable richtext policy: `article`, `discussion`, and
  `comment` profiles; strict tree/attribute/link validation; deterministic
  normalization; one escaped semantic HTML renderer; and one plain-text
  projection.
- Own orchestration state, idempotency, audit records, and canonical URL/alias mappings for cross-domain flows.
- Own content dashboard post analytics snapshots (`ContentCountSnapshot` and
  `load_post_stats_snapshot`) so host GraphQL does not embed `nodes` SQL.
- Expose a port-based `ContentOrchestrationService` that delegates domain work through `ContentOrchestrationBridge`.
- Publish only orchestration-facing RBAC for `forum_topics:*` and `blog_posts:*`.

## Interactions

- Depends on `rustok-core` for permissions, events, and `SecurityContext`.
- Depends on `rustok-api` for shared tenant/auth/request and GraphQL helper contracts.
- Exposes only its shared canonical-route GraphQL query and content-owned
  GraphQL dataloaders; product CRUD GraphQL, REST, admin, and storefront entry
  points remain domain-owned.
- Used as a shared helper dependency by `rustok-blog`, `rustok-forum`,
  `rustok-comments`, and `rustok-pages`.
- Declares permissions via `rustok-core::Permission`.
- `ContentOrchestrationService` enforces orchestration permissions from
  `AuthContext.permissions`, persists idempotency/audit state, and publishes
  orchestration events. Runtime adapters for domain conversions live outside the
  shared helper layer and implement `ContentOrchestrationBridge`.
- `rustok-content-orchestration` owns the runtime bridge implementation and its
  live GraphQL mutations for `topic ↔ post`, `split_topic`, and `merge_topics`.
- `apps/server` only composes the owner-provided GraphQL roots, dataloaders,
  and dashboard post analytics helper.

- Conversion flows persist typed redirect/canonical state in
  `content_canonical_urls` and `content_url_aliases` and publish
  `CanonicalUrlChanged` / `UrlAliasPurged` through the outbox contract.

Richtext policy is currently a target foundation. Owner write/read paths still
need the atomic Blog/Forum/Comments cutover before this policy becomes a
production runtime gate; no legacy path is being treated as a supported target
contract.

## Entry points

- `ContentModule`
- `ContentOrchestrationService`
- `ContentOrchestrationBridge`
- `load_post_stats_snapshot`
- `ContentCountSnapshot`
- `graphql::ContentQuery` (feature `graphql`)
- `graphql::{NodeLoader, NodeTranslationLoader, NodeBodyLoader}` (feature `graphql`)
- `CategoryService`
- content DTO and entity re-exports
- `richtext::{RichTextProfile, validate_and_normalize, render_html, plain_text}`

`NodeService` remains available only under `rustok-content::services` as a
shared-node helper surface. It is intentionally no longer part of the top-level
crate entry points.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
