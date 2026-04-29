# rustok-blog-admin

Leptos admin UI package for the `rustok-blog` module.

## Responsibilities

- Exposes the blog admin root view used by `apps/admin`.
- Stays module-owned: blog-specific admin UI does not live in `apps/admin`.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns the standard GraphQL-first blog CRUD flow: list/create/edit/update/publish/archive/delete.
- Embeds owner-side post SEO editing through `rustok-seo-admin-support` instead of relying on a central SEO entity editor.

## Entry Points

- `BlogAdmin` — root admin page component for the module.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, and `nav_label` for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/blog` through the generic module page route.
- Uses the `rustok-blog` GraphQL contract directly, plus shared Leptos host libraries.
- Treats a missing `posts` GraphQL contract in reduced server builds as an unavailable list surface
  and renders the normal empty state instead of surfacing a dashboard-level error.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-admin-support` for explicit post SEO authoring.
- Must keep GraphQL/API assumptions aligned with the module backend crate.

## Documentation

- See [platform docs](../../../docs/index.md).
