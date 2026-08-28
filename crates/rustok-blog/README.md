# rustok-blog

## Purpose

`rustok-blog` owns the Blog domain: posts, Blog Category membership/settings,
Blog-owned post-term relations, and comment integration via `rustok-comments`.
Shared vocabulary and canonical Blog Category localized identity are provided by
`rustok-taxonomy` through explicit owner boundaries.

## Responsibilities

- Provide `BlogModule` metadata for the runtime registry.
- Own Blog post lifecycle, Blog Category membership/settings/revision, SEO, and
  Blog-local orchestration.
- Own Blog GraphQL and REST transport adapters alongside domain services,
  including comment moderation and category CRUD under `/api/blog/categories`.
- Keep REST handlers on narrow `BlogHttpRuntime` state; the manifest-declared
  Axum router builds it from `HostRuntimeContext` and the host transactional
  event bus.
- Publish module-owned Leptos admin/storefront packages for installable UI
  surfaces.
- Publish schema-driven tenant settings through `rustok-module.toml`.
- Publish separate typed RBAC resources: `blog_posts:*` and
  `blog_categories:*`.
- Keep Blog Category commands synchronized with canonical Taxonomy Category
  state without restoring retired Blog Category translation storage.

## Category Taxonomy boundary

The former Blog-owned Category Translation target has been retired.
`BlogCategoryTranslationTargetProvider`, its owner change journal, its live
translation mirror and its provider-era test/evidence sources are not production
entry points.

The current boundary is:

- `rustok-taxonomy` owns canonical Blog Category localized copy and route
  history;
- Blog Category reads and mutation responses project canonical Taxonomy state;
- Blog Category create/update and hierarchy commands synchronize Taxonomy in the
  owner transaction;
- Blog Category delete delegates canonical lifecycle cleanup to Taxonomy;
- `blog_categories` remains Blog-owned for module membership, settings, owner
  revision and local command invariants;
- historical migration `000020` may use the crate-private donor translation
  entity during upgrade, after which `000021` irreversibly removes
  `blog_category_translations` and `blog_translation_changes` once same-ID
  Taxonomy ownership is proven.

Do not reintroduce a second `blog/category` Translation provider or direct
localized Blog Category storage. Translation-control-plane work for Category
copy must use the canonical Taxonomy owner contract.

## Interactions

- Depends on `rustok-channel` for channel-aware public Blog read gating.
- Depends on `rustok-content` for shared content helpers and cross-domain
  orchestration primitives.
- Depends on `rustok-comments` for comment threads, comment bodies, and generic
  comment lifecycle.
- Blog comment writes consume `RichTextDocument`; moderation reads consume the
  Comments-owned `RichTextView` and plain-text projection.
- Blog article writes accept the shared `RichTextDocument`; the owner applies
  the fixed `article` profile and persists canonical root JSON.
- Routes comment reads, create/update/delete, and moderation through the public
  `CommentsThreadPort`; Blog does not call `CommentsService` directly.
- Depends on `rustok-taxonomy` for the shared tag dictionary and canonical Blog
  Category copy/hierarchy projection while keeping `blog_post_tags` Blog-owned.
- Depends on `rustok-core` for module contracts, permissions, and
  `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter
  contracts.
- Used by `apps/server` through generated GraphQL composition and a
  manifest-declared Axum router mount.
- Used by `apps/admin` and `apps/storefront` through manifest-driven Leptos
  package composition.
- Public Blog reads honor channel module bindings and typed post visibility
  allowlists; authenticated/admin flows bypass the public channel gate.
- Post adapters validate `blog_posts:*`; category adapters validate only
  `blog_categories:*`. Catalog `categories:*` and `blog_posts:*` do not
  authorize Blog Category operations.
- Blog services re-validate RBAC locally.
- `CategoryService::new(db, event_bus)` is the Category service constructor; the
  required `TransactionalEventBus` keeps owner mutation and Search reindex
  publication in the same transaction.

## Entry points

- `BlogModule`
- `PostService`
- `CommentService`
- `CategoryService`
- `TagService`
- `graphql::BlogQuery`
- `graphql::BlogMutation`
- `controllers::axum_router`
- `admin::BlogAdmin`
- `storefront::BlogView`

## Docs

- [Module docs](./docs/README.md)
- [Current implementation cursor](./docs/implementation-plan-current.md)
- [Platform docs index](../../docs/index.md)
