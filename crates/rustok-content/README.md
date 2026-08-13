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
- Under the opt-in `richtext-assets` server feature, expose the canonical
  manifest-selected frame HTML, Leptos adapter, script, and stylesheet through
  same-origin `/richtext/frame` routes with content-derived cache validators.
- Own orchestration state, idempotency, audit records, and canonical URL/alias mappings for cross-domain flows.
- Own content dashboard post analytics snapshots (`ContentCountSnapshot` and
  `load_post_stats_snapshot`) so host GraphQL does not embed `nodes` SQL.
- Expose a port-based `ContentOrchestrationService` that delegates domain work through `ContentOrchestrationBridge`.
- Publish only orchestration-facing RBAC for `forum_topics:*` and `blog_posts:*`.

## Interactions

- Depends on `rustok-core` for permissions, events, and `SecurityContext`.
- Depends on `rustok-api` for shared tenant/auth/request and GraphQL helper contracts.
- Exposes only its shared canonical-route GraphQL query; product CRUD GraphQL,
  REST, admin, and storefront entry points remain domain-owned.
- Used as a shared helper dependency by `rustok-blog`, `rustok-forum`,
  `rustok-comments`, and `rustok-pages`.
- Declares permissions via `rustok-core::Permission`.
- `ContentOrchestrationService` enforces orchestration permissions from
  `AuthContext.permissions`, persists idempotency/audit state, and publishes
  orchestration events. Runtime adapters for domain conversions live outside the
  shared helper layer and implement `ContentOrchestrationBridge`.
- `rustok-content-orchestration` owns the runtime bridge implementation and its
  live GraphQL mutations for `topic ↔ post`, `split_topic`, and `merge_topics`.
- `apps/server` only composes the owner-provided GraphQL roots and dashboard
  post analytics helper.

- Conversion flows persist typed redirect/canonical state in
  `content_canonical_urls` and `content_url_aliases` and publish
  `CanonicalUrlChanged` / `UrlAliasPurged` through the outbox contract.

Richtext policy is the production runtime gate for Blog, Forum, and Comments.
Their owner services select fixed profiles and keep locale in owner rows. The
obsolete core richtext/format helpers, generic `NodeService`, generic category
CRUD/runtime entities, and unused generic content entity DataLoaders are removed
from the public runtime surface. Retained historical category migrations remain
only for database compatibility; Blog/Forum and other domain modules own their
live category aggregates and hierarchy rules. Pages accepts only its
owner-selected Page Builder document format. Blog/Comments initial schemas are
canonical and their corrective migration/conversion artifacts are absent.

## Entry points

- `ContentModule`
- `ContentOrchestrationService`
- `ContentOrchestrationBridge`
- `load_post_stats_snapshot`
- `ContentCountSnapshot`
- `graphql::ContentQuery` (feature `graphql`)
- owner-neutral content DTO/entity helpers documented by `CRATE_API.md`
- `richtext::{RichTextProfile, validate_and_normalize, render_html, plain_text}`
- `richtext_assets::router` (feature `richtext-assets`)

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
