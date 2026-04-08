# rustok-blog

## Purpose

`rustok-blog` owns the blog domain with module-owned post/category storage, blog-owned post-term relations, shared taxonomy-backed tag vocabulary, and comment integration via `rustok-comments`.

## Responsibilities

- Provide `BlogModule` metadata for the runtime registry.
- Own blog-specific post lifecycle, SEO, and localized blog orchestration.
- Own blog GraphQL and REST transport adapters alongside the domain services.
- Publish module-owned Leptos admin/storefront packages for installable UI surfaces.
- Publish schema-driven tenant settings through `rustok-module.toml`, including curated option sets for admin forms.
- Publish the typed `blog_posts:*` RBAC surface.

## Interactions

- Depends on `rustok-channel` for the second public channel-aware gating proof point on blog read paths.
- Depends on `rustok-content` only for shared content helpers and cross-domain orchestration primitives.
- Depends on `rustok-comments` for comment threads, comment bodies, and generic comment lifecycle.
- Depends on `rustok-taxonomy` for the shared tag dictionary while keeping `blog_post_tags` blog-owned.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- Used by `apps/admin` and `apps/storefront` through manifest-driven Leptos package composition.
- Public blog read paths can now honor `channel_module_bindings` when a request carries an active
  channel through `RequestContext`; authenticated/admin flows intentionally bypass that pilot gate.
- Public published blog read paths also honor typed `blog_post_channel_visibility`
  allowlists behind the existing `channelSlugs` wire contract; empty allowlists
  stay globally visible, while authenticated/admin flows intentionally bypass
  this publication gate.
- Declares permissions via `rustok-core::Permission`.
- Transport adapters validate `blog_posts:*` against `AuthContext.permissions`, then pass
  a permission-aware `SecurityContext` into blog services.
- Blog services now re-validate RBAC locally for posts, categories, and tags, and customer
  read paths are restricted to published posts even when the transport layer is authenticated.

## Entry points

- `BlogModule`
- `PostService`
- `CommentService`
- `CategoryService`
- `TagService`
- `graphql::BlogQuery`
- `graphql::BlogMutation`
- `controllers::routes`
- `admin::BlogAdmin`
- `storefront::BlogView`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
