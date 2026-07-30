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
- Owns blog CRUD through a module transport facade: SSR/hydrate selects native `#[server]` functions, while standalone CSR selects the parallel GraphQL contract.
- Owns a separate comment-moderation slice through `admin/src/moderation.rs`; selecting a post loads its non-deleted owner queue and supports approve/spam/trash actions through the selected native or GraphQL transport.
- Paginates the moderation queue with bounded `page`/`per_page` inputs, resets page state when the selected post changes, and prevents navigation outside the server-reported total.
- Keeps moderation separate from the post detail query so editors without `blog_posts:manage` retain normal CRUD behavior and reduced GraphQL builds can degrade only the moderation panel.
- Embeds owner-side post SEO editing through `rustok-seo-admin-support` instead of relying on a central SEO entity editor.
- Keeps Leptos render/bind code in `admin/src/ui/leptos.rs`; native server functions, GraphQL adapters, and moderation GraphQL queries stay under `admin/src/transport/`.
- Mounts the shared sandboxed `@rustok/richtext` frame through the thin WASM lifecycle bridge only during browser hydration. SSR emits the iframe markup and never executes browser/WASM code.

## Entry Points

- `BlogAdmin` — composed root admin page containing the existing post editor and the selected-post moderation panel.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, and `nav_label` for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/blog` through the generic module page route.
- Uses `transport/native_server_adapter.rs` for the Leptos SSR/hydrate internal path and keeps `transport/graphql_adapter.rs` plus `transport/moderation_adapter.rs` as the parallel public/headless path. A failed mutation is never retried through another protocol.
- Treats a missing `posts` GraphQL contract in reduced server builds as an unavailable list surface and renders the normal empty state instead of surfacing a dashboard-level error.
- Treats a missing `moderationComments`, `moderateComment`, or `BlogCommentModerationStatus` contract as a moderation-only unavailable state.
- The backend requires `blog_posts:manage`, current-tenant binding, and the Blog field-aware rate-limit policy before trusted owner-side comment reads or status changes.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-admin-support` for explicit post SEO authoring.
- Native and GraphQL adapters call the same Blog owner services and must keep their DTO, authorization, tenant, locale, and error semantics aligned.

## Documentation

- See [platform docs](../../../docs/index.md).
