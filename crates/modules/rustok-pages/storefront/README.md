# rustok-pages-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md) |
> [Public API Contract](CRATE_API.md)

## Purpose

`rustok-pages-storefront` publishes the Leptos storefront root view for the `rustok-pages` module.

## Responsibilities

- Export the module-owned `PagesView` root component for `apps/storefront`.
- Keep pages-specific storefront rendering inside the module boundary.
- Keep the storefront FFA split explicit: `core.rs` owns view-model/formatting helpers, `transport.rs` owns the module facade, and `src/ui/leptos.rs` owns Leptos render/bind code.
- Act as the canonical working storefront read-path for published pages.
- Render the published landing artifact of a page (the sanitized markup and stylesheet stored by the reviewed publish) directly through SSR HTML and scoped CSS via `PublishedLandingBody` and `StaticLandingStorefront`, without `<iframe>` wrappers and with full action runtime support.
- Never request, transport, or render the page body. The body is the Page Builder authoring document; `rustok-pages` returns it only to a principal that may save or publish it, and this package has no type that could carry it.

## Interactions

- Used by `apps/storefront` through manifest-driven generated wiring.
- Uses dual-path data access through `src/transport.rs`: native Leptos `#[server]` functions for monolith/hydrate builds and GraphQL for headless/CSR builds.
- Native `#[server]` path goes from the storefront host into `PageService` through `HostRuntimeContext`; DB and `TransactionalEventBus` are host-provided typed handles, so the package does not depend on a host framework.
- Both paths return the same `PageDetail { effective_locale, translation, landing }`. `landing` is a `PublishedLanding { locale, html, css, artifact_hash }`:
  - the native path loads it with `PageBuilderArtifactService::load_public_bound_artifact_with_fallback` for the locale the page resolved to; the artifact owner decides page status, channel visibility, the locale binding, and artifact integrity;
  - the GraphQL path selects `pageBySlug { landing { locale html css artifactHash } }`, which is resolved by the same owner call.
- `PublishedLandingBody` renders `html` and `css` without an iframe; `StaticLandingStorefront` scopes the stylesheet to the landing element and inlines `fly_browser::FLY_ACTIONS_JS` when interactive actions (`data-fly-action`) are present.
- A page without a published landing (never published through the reviewed publish, or not published for the request locale or channel) renders the `pages.body.empty` state. There is no fallback content source.
- Keeps the pages module GraphQL read contract active in parallel with the native path while rendering a slug-selected page and a small page directory.
- Follows the generic storefront host contract: slots plus locale-aware module routes built from `UiRouteContext::module_route_base()`.

## Entry points

- `PagesView` (re-exported from `src/ui/leptos.rs`)
- `PublishedLandingBody` (re-exported from `src/builder.rs`)
- `PublishedLanding` (re-exported from `src/model.rs`)

## Known Limitations / Pending Implementation

- The cached native storefront snapshot (`StorefrontPagesData`, 60 s TTL) changed shape when the page body was replaced by `landing`. `PageDetail` rejects unknown fields, so an entry written by an older release is a decode error and is reloaded from the source. An older release reading an entry written by this one shows the page without content until the entry expires; avoid serving both releases from one cache for longer than the TTL.
- The selected-page card is a module view, not a route: embedded on its own, it shows the empty state for a page without a published landing. The route of such a page does not exist on either storefront: `resolve_storefront_page_route` answers `not_found` for it, for the canonical route and for its aliases, as the Next.js storefront route answers 404. The decision reads the landing binding on every page view.
