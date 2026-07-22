# rustok-search-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos storefront UI package for the `rustok-search` module.

## Responsibilities

- Exposes the search storefront root view used by `apps/storefront`.
- Keeps search-specific storefront UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Keeps the crate root wiring-only: `src/lib.rs` declares `core`, `transport`, and `ui`, while `src/ui/leptos.rs` owns the Leptos render adapter for `SearchView`.
- Provides the baseline route/slot scaffold for query input, suggestions, filters, and results.
- Keeps storefront result summary, preset, locale, item, source, score, snippet, and click presentation in framework-agnostic core view-model helpers.
- Uses native Leptos `#[server]` entry points in parallel with the GraphQL transport.
- Consumes canonical result URLs produced by `rustok-search::canonical_search_result_url`; the package does not parse indexed payloads or construct product, content, or Blog routes.
- Ships package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.

## Entry Points

- `SearchView` — root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses the shared `UiRouteContext` to read query-string state without leaking host-specific routing details, including locale-aware generic module routes.
- Runtime data access is build-profile-selected: native `#[server]` for monolith/hydrate builds and GraphQL for headless/CSR builds. GraphQL remains a first-class transport.
- Native Search mapping and GraphQL result serialization both delegate URL ownership to the Search core before returning the shared DTO.
- The transport facade returns the selected payload unchanged. There is no post-transport navigation enrichment, fallback route builder, or transport-local Blog slug parser.
- Invalid, missing, oversized, spoofed, or route-breaking Blog slugs remain non-navigable because the Search owner policy fails closed before serialization.
- Remains aligned with future storefront packages through the same Search API and normalized result model.
- Reads the effective locale from `UiRouteContext.locale` for visible chrome, empty states, and result helper copy.

## Documentation

- See [platform docs](../../../docs/index.md).
