# rustok-blog

## Purpose

`rustok-blog` owns the blog domain built on top of the content module.

## Responsibilities

- Provide `BlogModule` metadata for the runtime registry.
- Own blog-specific post lifecycle, SEO, and localized blog orchestration.
- Own blog GraphQL and REST transport adapters alongside the domain services.
- Publish module-owned Leptos admin/storefront packages for installable UI surfaces.
- Publish the typed `blog_posts:*` RBAC surface.

## Interactions

- Depends on `rustok-channel` for the second public channel-aware gating proof point on blog read paths.
- Depends on `rustok-content` for shared content storage and orchestration primitives.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- Used by `apps/admin` and `apps/storefront` through manifest-driven Leptos package composition.
- Public blog read paths can now honor `channel_module_bindings` when a request carries an active
  channel through `RequestContext`; authenticated/admin flows intentionally bypass that pilot gate.
- Public published blog read paths also honor metadata-based `channelSlugs` allowlists on posts for
  unauthenticated requests; empty allowlists stay globally visible, while authenticated/admin flows
  intentionally bypass this experimental publication gate.
- Declares permissions via `rustok-core::Permission`.
- Transport adapters validate `blog_posts:*` against `AuthContext.permissions`, then pass
  a permission-aware `SecurityContext` into blog services.

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
