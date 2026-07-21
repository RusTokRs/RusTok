# rustok-blog

## Purpose

`rustok-blog` owns the blog domain with module-owned post/category storage, blog-owned post-term relations, shared taxonomy-backed tag vocabulary, and comment integration via `rustok-comments`.

## Responsibilities

- Provide `BlogModule` metadata for the runtime registry.
- Own blog-specific post lifecycle, categories, SEO, and localized orchestration.
- Own Blog GraphQL and REST transport adapters alongside domain services, including comment moderation endpoint `POST /api/blog/comments/{id}/moderate` and category CRUD under `/api/blog/categories`.
- Keep REST handlers on narrow `BlogHttpRuntime` state; the manifest-declared Axum router builds it from `HostRuntimeContext` and the host transactional event bus.
- Publish module-owned Leptos admin/storefront packages for installable UI surfaces.
- Publish schema-driven tenant settings through `rustok-module.toml`, including curated option sets for admin forms.
- Publish separate typed RBAC resources: `blog_posts:*` and `blog_categories:*`.

## Interactions

- Depends on `rustok-channel` for the second public channel-aware gating proof point on Blog read paths.
- Depends on `rustok-content` only for shared content helpers and cross-domain orchestration primitives.
- Depends on `rustok-comments` for comment threads, comment bodies, and generic comment lifecycle.
- Routes comment reads, update, and moderation through the public `CommentsThreadPort`, including create/delete; no Blog code calls `CommentsService` directly. Comments lifecycle events are consumed by Blog's idempotent reply-count projection, which atomically publishes `BlogPostUpdated`.
- Depends on `rustok-taxonomy` for the shared tag dictionary while keeping `blog_post_tags` Blog-owned.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Used by `apps/server` through generated GraphQL composition and a manifest-declared Axum router mount.
- Used by `apps/admin` and `apps/storefront` through manifest-driven Leptos package composition.
- Public Blog read paths honor `channel_module_bindings` when a request carries an active channel through `RequestContext`; authenticated/admin flows bypass the public channel gate.
- Public published Blog reads honor typed `blog_post_channel_visibility` allowlists behind the `channelSlugs` wire contract; empty allowlists stay globally visible.
- Post adapters validate `blog_posts:*`; category adapters validate only `blog_categories:*`.
- Catalog `categories:*` and `blog_posts:*` do not authorize Blog category operations.
- Blog services re-validate RBAC locally. Customer post reads are restricted to published posts.
- `CategoryService::new(db, event_bus)` is the only category service constructor. The required `TransactionalEventBus` keeps category mutation and Search reindex publication in the same transaction.

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
- [Platform docs index](../../docs/index.md)
