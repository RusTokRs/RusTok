# rustok-forum-admin

Leptos admin UI package for the `rustok-forum` module.

## Responsibilities

- Exposes the forum admin root view used by `apps/admin`.
- Keeps forum-specific admin UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns a real REST-backed admin slice for category/topic CRUD and reply previews.
- Presents the admin workflow as a NodeBB-inspired moderation workspace with category rail, topic feed, and thread inspector.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.
- Embeds owner-side SEO panels for forum categories and topics through `rustok-seo-admin-support`.

## Entry Points

- `ForumAdmin` - root admin page component for the module.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, `nav_label`, and nested admin pages for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/forum`.
- Uses the `rustok-forum` HTTP contract directly because REST currently exposes complete detail endpoints for topics/categories/replies, while GraphQL is still list-oriented on the read path.
- Keeps the richer forum-specific layout inside the module crate so the host stays generic while `/modules/forum` feels like a native community console.
- Keeps forum SEO ownership inside the forum package through real category/topic SEO panels rather than a delegated central SEO editor.
- Reads the effective locale from `UiRouteContext.locale`; package-owned translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
