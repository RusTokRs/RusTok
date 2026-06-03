# rustok-region-storefront

Leptos storefront UI package for the `rustok-region` module.

## Responsibilities

- Exposes the region storefront root view used by `apps/storefront`.
- Keeps region-specific storefront UI inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Provides a public read-side route for region, country, currency, and tax baseline discovery.
- Exposes the typed region tax provider snapshot alongside currency and tax
  baseline data.
- Uses native Leptos `#[server]` entry points in parallel with the existing GraphQL transport through an explicit `transport/` facade with native and GraphQL adapters. The facade returns a typed error envelope that preserves failed-path and fallback evidence.
- Keeps route/tax/country summary formatting, selected-region resolution, route/query state (`RegionRouteState`, `RegionRouteSelectionUpdate`, `SELECTED_REGION_QUERY_KEY`), error status/view-model mapping, `RegionErrorDomEvidence`, and selected-region card/metric and rail list view-model helpers in `storefront/src/core.rs`, outside the Leptos render layer and transport adapters. The Leptos render/bind adapter lives in `storefront/src/ui/leptos.rs`; `storefront/src/lib.rs` only wires modules and re-exports `RegionView`. Host-visible error statuses are `native_unavailable` (`region.error.status.nativeUnavailable`) and `fallback_unavailable` (`region.error.status.fallbackUnavailable`), and the Leptos error adapter exposes them as `data-region-error-status` / `data-region-error-locale-key`. Region rail links also publish route/query evidence as `data-region-route-query-key` / `data-region-route-query-value`. The SSR smoke tests `region_error_message_ssr_exposes_host_visible_dom_evidence` and `region_rail_ssr_exposes_route_query_dom_evidence` render the adapters and verify status plus route/query attributes.
- Ships package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.

## Entry Points

- `RegionView` - root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses `RegionService` directly on the SSR path and falls back to the existing `storefrontRegions` GraphQL contract when native transport is unavailable.
- Reads the effective locale from `UiRouteContext.locale`; route/query semantics remain core-owned via `SELECTED_REGION_QUERY_KEY` and `RegionRouteState`, while the Leptos adapter only supplies `UiRouteContext::module_route_base()` for host path wiring.

## Documentation

- See [platform docs](../../../docs/index.md).
