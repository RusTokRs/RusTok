# rustok-ui-i18n

## Purpose

`rustok-ui-i18n` provides framework-agnostic UI message catalog helpers for
RusToK module-owned UI packages and future UI adapters.

## Architecture & Modules

The crate is structured into focused domain modules:

- `locale`: Unicode Language Identifier normalization (`normalize_locale_tag`), canonical admin locale resolution (`normalize_admin_locale`), and structured fallback candidate chains (`locale_candidates`).
- `bundle`: Concurrent Project Fluent (`.ftl`) bundle (`build_fluent_bundle`) and catalog (`build_fluent_catalog`, `try_build_fluent_catalog`, `bundle::build_fluent_catalog_report`, `FluentCatalog`) construction with Unicode bidi isolation enabled for interpolated values.
- `messages`: Core thread-safe UI message facade (`UiMessages`), fail-closed prepared runtime (`PreparedUiMessages`), borrowed translator (`UiTranslator`), prepared per-locale translator (`UiLocaleTranslator`), stack-buffered safe kebab-case key conversion (`with_kebab_key`), strict/lenient candidate resolution (`try_resolve_fluent_message`, `resolve_fluent_message`), and cached lazy-initialization diagnostics.
- `error`: Typed errors (`BundleBuildError`, `I18nError`) for locale, catalog, lookup, and formatting failures.
- `macros`: Ergonomic macros (`declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`).

## Responsibilities

- Manage Project Fluent (`.ftl`) message catalogs with natural grammar, selectors, pluralization, and parameter interpolation.
- Provide thread-safe concurrent bundle management via `UiMessages` (`Send + Sync`).
- Preserve Project Fluent Unicode directional isolation around interpolated values so mixed LTR/RTL UI text renders safely.
- Resolve message keys from the host-provided effective locale with stack buffering for dotted keys up to 128 bytes.
- Apply a structured platform UI fallback chain (exact locale -> variants removed as one layer -> region -> script -> language -> default locale -> `"en"` -> fallback string) without depending on Leptos, Dioxus, Next.js, or host routing.
- Provide strict startup preparation (`UiMessages::prepare`) and fail-soft initialization diagnostics without rebuilding the lazy catalog.
- Keep request/catalog/default locale normalization bounded on raw input before trim/normalization work.
- Keep UI i18n catalog logic out of `rustok-api` and framework-specific crates.
- Maintain compile-time portability for `wasm32-unknown-unknown` (pure in-memory, zero runtime filesystem access).

## Locale Model

Catalog keys are represented by `unic_langid::LanguageIdentifier`: language, optional script,
optional region, and variants. Unicode/private-use extensions are not retained as part of catalog
identity. Callers that require extension-aware locale semantics must keep that policy in the host layer
until the locale model is deliberately widened.

The Rust locale-input policy rejects raw locale strings longer than 64 bytes before trimming or
underscore normalization. Runtime lookup, direct bundle construction, strict/lenient catalog
construction, and default-locale validation use the same contract. Oversized diagnostics retain only
length metadata and never copy or log the full untrusted payload. `@rustok/next-fluent` mirrors the
raw 64-code-unit boundary for its locale entry points and keeps oversized configuration diagnostics bounded.

## Key Identity and Catalog Collisions

Application-facing dotted keys are a convenience spelling for Fluent kebab IDs: `account.profile.title`
and `account-profile-title` intentionally resolve to the same message identity. Treat those spellings as aliases,
not as two independent keys; a module must not assign different semantics to the dotted and kebab forms.

Catalog construction has three explicit modes:

- `try_build_fluent_catalog` is fail-closed: malformed/oversized locale input, invalid resources, and duplicate normalized locale keys are returned as typed errors.
- `build_fluent_catalog` is the lenient rendering convenience path: invalid catalog entries are logged and skipped, while the first bundle for a normalized locale key wins.
- `bundle::build_fluent_catalog_report` uses the same lenient behavior but additionally returns typed diagnostics for every skipped entry so startup health code does not need to scrape logs.

`UiMessages` caches the lenient report once. `initialization_diagnostics()` exposes both skipped-entry
errors and invalid/oversized/missing default-locale configuration from that same initialization; it does
not rebuild the catalog solely to recover diagnostics.

Within an individual Fluent resource, message/resource conflicts are handled by Project Fluent's
`add_resource` validation and surface as `BundleBuildError::AddResource`; this crate does not silently invent
an override policy for Rust catalogs.

## Entry Points

- `UiMessages`
- `PreparedUiMessages`
- `UiTranslator`
- `UiLocaleTranslator`
- `BundleBuildError`
- `I18nError`
- `declare_module_i18n!`
- `fluent_args!`
- `t!`
- `module_t!`
- `normalize_admin_locale`
- `normalize_locale_tag`
- `locale_candidates`
- `build_fluent_bundle`
- `build_fluent_catalog`
- `try_build_fluent_catalog`
- `bundle::build_fluent_catalog_report`
- `bundle::FluentCatalogBuildReport`
- `resolve_fluent_message`
- `try_resolve_fluent_message`
- `with_kebab_key`

## Interactions

- Module-owned UI packages use this crate from local `i18n.rs` files via `declare_module_i18n!`.
- Host/runtime code owns effective locale selection; this crate only resolves messages for a supplied locale.
- `@rustok/next-fluent` uses the same Project Fluent bidi-safe default, bounded locale-input policy, and structured script/region/variant fallback shape for core Rust/Next parity.

## Boundary Rules

- Do not add Leptos, Dioxus, Axum, GraphQL, cookie, header, query, or routing dependencies.
- Do not select the user's locale here; consume the host-provided effective locale.
- Do not add runtime filesystem scanning or environment lookups in production paths.
- Do not add module-specific message keys or business copy to this crate.

## Docs

- [Crate docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)
- [Module UI package implementation guide](../../../docs/UI/module-package-implementation.md)
