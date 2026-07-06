# rustok-forum-admin

> **For contributors and AI agents — read before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-forum` module.

## Responsibilities

- Exposes the forum admin root view used by `apps/admin`.
- Keeps forum-specific admin UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns a GraphQL-first admin transport slice for category/topic CRUD and reply previews, with REST fallback kept behind the module transport facade.
- Presents the admin workflow as a NodeBB-inspired moderation workspace with category rail, topic feed, and thread inspector.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.
- Embeds owner-side SEO panels for forum categories and topics through `rustok-seo-admin-support`.

## Entry Points

- `ForumAdmin` - root admin page component for the module.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, `nav_label`, and nested admin pages for host composition.

## FFA structure

- `admin/src/core.rs` owns framework-agnostic tag parsing, category-filter normalization, status/count helpers, category/topic form snapshots, submit validation, category/topic card view-model mapping, category sidebar mapping, and reply-stack view-model mapping with exact busy item matching.
- `admin/src/transport.rs` is the module-owned facade over GraphQL-first admin transport and REST fallback adapters.
- `admin/src/ui/leptos.rs` is the explicit Leptos render/effect adapter and does not own draft validation policy.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/forum`.
- Uses the `rustok-forum` GraphQL admin contract for category/topic CRUD and reply previews, while keeping REST fallback hidden behind the transport facade.
- Keeps the richer forum-specific layout inside the module crate so the host stays generic while `/modules/forum` feels like a native community console.
- Keeps forum SEO ownership inside the forum package through real category/topic SEO panels rather than a delegated central SEO editor.
- Reads the effective locale from `UiRouteContext.locale`; package-owned translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
