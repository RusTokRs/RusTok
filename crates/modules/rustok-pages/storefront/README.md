# rustok-pages-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

## Purpose

`rustok-pages-storefront` publishes the Leptos storefront root view for the `rustok-pages` module.

## Responsibilities

- Export the module-owned `PagesView` root component for `apps/storefront`.
- Keep pages-specific storefront rendering inside the module boundary.
- Keep the storefront FFA split explicit: `core.rs` owns view-model/formatting helpers, `transport.rs` owns the module facade, and `src/ui/leptos.rs` owns Leptos render/bind code.
- Act as the canonical working storefront read-path for published pages.
- Render published Page Builder landing pages directly through SSR HTML and scoped CSS via `PageBuilderPageBody` and `StaticLandingStorefront`, eliminating `<iframe>` wrappers on storefront views while retaining full action runtime support.

## Interactions

- Used by `apps/storefront` through manifest-driven generated wiring.
- Uses dual-path data access through `src/transport.rs`: native Leptos `#[server]` functions for monolith/hydrate builds and GraphQL for headless/CSR builds.
- Native `#[server]` path goes from the storefront host into `PageService` through `HostRuntimeContext`; DB and `TransactionalEventBus` are host-provided typed handles, so the package does not depend on a host framework.
- For published landing pages, `native_server_adapter` loads the immutable pre-compiled artifact, returning `fly_artifact_payload` (`StaticLandingBodyPayload { html, css, artifact_url }`).
- `PageBuilderPageBody` directly renders the pre-compiled HTML and CSS without an iframe, while inlining `fly_browser::FLY_ACTIONS_JS` when interactive actions (`data-fly-action`) are present.
- Keeps the pages module GraphQL read contract active in parallel with the native path while rendering a slug-selected page and a small page directory.
- Follows the generic storefront host contract: slots plus locale-aware module routes built from `UiRouteContext::module_route_base()`.

## Entry points

- `PagesView` (re-exported from `src/ui/leptos.rs`)
- `PageBuilderPageBody` (re-exported from `src/builder.rs`)
- `STATIC_LANDING_PAYLOAD_BODY_FORMAT` (`"fly_artifact_payload"`)
