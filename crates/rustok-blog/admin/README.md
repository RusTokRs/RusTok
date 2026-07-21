# rustok-blog-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-blog` module.

## Responsibilities

- Exposes the blog admin root view used by `apps/admin`.
- Stays module-owned: blog-specific admin UI does not live in `apps/admin`.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns the standard GraphQL-first blog CRUD flow through a module-owned `admin/src/transport/mod.rs` facade backed by `admin/src/transport/graphql_adapter.rs`: list/create/edit/update/publish/archive/delete.
- Owns a separate comment-moderation slice through `admin/src/moderation.rs` and `transport/moderation_adapter.rs`; selecting a post loads its non-deleted owner queue and supports approve/spam/trash actions through `moderateComment`.
- Paginates the moderation queue with bounded GraphQL `page/perPage` variables, resets page state when the selected post changes, and prevents navigation outside the server-reported total.
- Keeps moderation separate from the post detail query so editors without `blog_posts:manage` retain normal CRUD behavior and reduced GraphQL builds can degrade only the moderation panel.
- Embeds owner-side post SEO editing through `rustok-seo-admin-support` instead of relying on a central SEO entity editor.
- Keeps Leptos render/bind code in `admin/src/ui/leptos.rs`; the crate root composes that editor with `BlogModerationPanel`, while transport-specific GraphQL code stays under `admin/src/transport/`.

## Entry Points

- `BlogAdmin` — composed root admin page containing the existing post editor and the selected-post moderation panel.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, and `nav_label` for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/blog` through the generic module page route.
- Uses the `rustok-blog` GraphQL contract via the package transport facade. CRUD delegates to `transport/graphql_adapter.rs`; moderation delegates independently to `transport/moderation_adapter.rs`.
- Treats a missing `posts` GraphQL contract in reduced server builds as an unavailable list surface and renders the normal empty state instead of surfacing a dashboard-level error.
- Treats a missing `moderationComments`, `moderateComment`, or `BlogCommentModerationStatus` contract as a moderation-only unavailable state.
- The backend requires `blog_posts:manage`, current-tenant binding, and the Blog field-aware rate-limit policy before trusted owner-side comment reads or status changes.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-admin-support` for explicit post SEO authoring.
- Must keep GraphQL/API assumptions aligned with the module backend crate.

## Documentation

- See [platform docs](../../../docs/index.md).
