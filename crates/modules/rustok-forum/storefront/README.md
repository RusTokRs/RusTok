# rustok-forum-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos storefront UI package for the `rustok-forum` module.

## Responsibilities

- Exposes the forum storefront root view used by `apps/storefront`.
- Keeps forum-specific storefront UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns dual-path storefront data access for category feeds, selected threads, reply rendering, authenticated visible-topic unread state, and canonical topic-route admission.
- Adds native Leptos `#[server]` calls while keeping GraphQL as a required parallel fallback.
- Builds topic-card links with the localized canonical route `/{locale}/forum/t/{short_id}/{slug}` instead of UUID query links.
- Presents the module as a NodeBB-inspired public discussion surface.
- Ships package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.

## Entry Points

- `ForumView` - root storefront view rendered from the host storefront slot registry.
- `resolve_storefront_topic_route` - selected-path canonical/redirect resolver used by the storefront host.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses build-profile-selected native `#[server]` calls with GraphQL selected path and shared host libraries such as `UiRouteContext`.
- Keeps GraphQL `forumStorefrontTopicRoute` and native `forum/storefront-topic-route` aligned on `ForumTopicRouteService` plus `ForumTopicAudienceReadService`.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.
- Reads the effective locale from `UiRouteContext.locale` for chrome copy and renders server-sanitized richtext projections.
- Keeps public category/topic/reply reads as the compatibility baseline. Authenticated requests enrich only the already storefront-visible topic IDs with the canonical Forum unread owner projection.
- Rechecks the canonical route target through the exact category/topic audience owner before disclosure or SSR composition.
- Rechecks storefront visibility before marking the selected topic read. Anonymous requests never create read rows or receive synthetic unread values.
- Degrades to the public feed only when authentication or the required Forum topic permission is absent; network, HTTP, persistence and domain failures remain explicit.
- Hides deleted-route tombstones until Forum owns a visibility-authorized disclosure snapshot; the Rust host therefore returns the same public `404` for missing, hidden, deleted and `GONE` routes.
- Does not expose category-subtree or tenant-wide mark-read commands because those owner scopes are not yet narrowed to the storefront channel-visible topic set.

## Documentation

- [FORUM-24I canonical topic route mount](../docs/forum-24i-topic-route-storefront-mount.md)
- See [platform docs](../../../docs/index.md).

## FFA boundary

The package keeps runtime-independent storefront policy in `src/core.rs`: canonical topic hrefs, rich-content summaries, count/slug labels, category/topic card view-models, unread badge/card mapping, accent fallback, and stable status badge class mapping. `src/transport/` remains the build-profile-selected native/GraphQL selected-path facade, while `src/ui/leptos.rs` is the explicit Leptos adapter. The host receives only visibility-admitted route DTOs and owns HTTP composition; it does not read Forum storage or alias history. The fast non-compiling guardrails are `npm run verify:forum:storefront-boundary` and `node scripts/verify/verify-forum-topic-route-storefront-mount.mjs`.
