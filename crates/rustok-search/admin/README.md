# rustok-search-admin

Leptos admin UI package for the `rustok-search` module.

## Responsibilities

- Exposes the search admin root view used by `apps/admin`.
- Keeps search-specific admin UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Provides a scaffold for overview, playground, engines, dictionaries, and analytics pages.

## Entry Points

- `SearchAdmin` — root admin page component for the module.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/search`.
- Uses shared `UiRouteContext` so nested module-owned pages stay host-agnostic.
- Consumes the host-provided effective locale from `UiRouteContext.locale`; package-owned strings live in `admin/locales/en.json` and `admin/locales/ru.json`.
- Declares its package-owned translation bundles in `rustok-module.toml` through `[provides.admin_ui.i18n]`.
- Uses native-first Leptos `#[server]` functions for bootstrap, preview, diagnostics, dictionaries, analytics, settings, and rebuild flows.
- Keeps the existing GraphQL transport as a parallel fallback path; native server functions do not replace `/api/graphql`.

## Documentation

- See [platform docs](../../../docs/index.md).
