# rustok-cache-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-cache` module.

## Responsibilities

- Exposes the cache management root view used by `apps/admin`.
- Keeps cache diagnostics and configuration UI inside the module-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Implements the FFA (Fluid Frontend Architecture) three-layer split (`core`, `transport`, `ui/leptos` + `i18n`).
- Keeps framework-agnostic form state parsing, DTO transformation, diagnostics view models, and display label resolution in `src/core.rs` (0 framework dependencies).
- Ships package-owned `admin/locales/en.ftl` and `admin/locales/ru.ftl` bundles declared through `[provides.admin_ui.i18n]` and accessed via `rustok-ui-i18n`.
- Uses dual-path transport in `src/transport.rs` (native server adapter for monolith SSR/hydrate + GraphQL client for CSR/headless).
- Keeps Leptos render/bind code in `src/ui/leptos.rs`; `src/lib.rs` only wires modules and re-exports `CacheAdmin`.

## Entry Points

- `CacheAdmin` - root admin view rendered by `apps/admin`.

## Interactions

- Consumed by `apps/admin` mounted at `/cache`.
- Reads cache diagnostics and writes settings via platform settings service / GraphQL facade.
- Reads effective UI locale from `UiRouteContext.locale` passed by the host.

## Documentation

- See [platform docs](../../../../docs/index.md).
