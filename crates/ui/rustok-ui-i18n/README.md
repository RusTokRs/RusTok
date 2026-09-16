# rustok-ui-i18n

## Purpose

`rustok-ui-i18n` provides framework-agnostic UI message catalog helpers for
RusToK module-owned UI packages and future UI adapters.

## Architecture & Modules

The crate is structured into focused domain modules:

- `locale`: Unicode Language Identifier normalization (`normalize_locale_tag`), canonical admin locale resolution (`normalize_admin_locale`), and fallback candidate chains (`locale_candidates`).
- `bundle`: Concurrent Project Fluent (`.ftl`) bundle (`build_fluent_bundle`) and catalog (`build_fluent_catalog`, `try_build_fluent_catalog`, `FluentCatalog`) construction with Unicode bidi isolation enabled for interpolated values.
- `messages`: Core thread-safe UI message facade (`UiMessages`), borrowed translator (`UiTranslator`), zero-allocation stack-buffered kebab-case key conversion (`with_kebab_key`), and strict/lenient candidate resolution (`try_resolve_fluent_message`, `resolve_fluent_message`).
- `error`: Typed errors (`BundleBuildError`, `I18nError`) for parse, resource, lookup, and formatting failures.
- `macros`: Ergonomic macros (`declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`).

## Responsibilities

- Manage Project Fluent (`.ftl`) message catalogs with natural grammar, selectors, pluralization, and parameter interpolation.
- Provide thread-safe concurrent bundle management via `UiMessages` (`Send + Sync`).
- Preserve Project Fluent Unicode directional isolation around interpolated values so mixed LTR/RTL UI text renders safely.
- Resolve message keys from the host-provided effective locale with zero-allocation stack buffering for dotted keys up to 128 bytes.
- Apply the platform UI fallback chain (regional/script/variant tag -> less-specific language identifier -> default locale -> `"en"` -> fallback string) without depending on Leptos, Dioxus, Next.js, or host routing.
- Provide locale normalization for catalog lookup through `unic_langid::LanguageIdentifier`.
- Provide boilerplate reduction macro (`declare_module_i18n!`) for module UI packages.
- Keep UI i18n catalog logic out of `rustok-api` and framework-specific crates.
- Maintain compile-time portability for `wasm32-unknown-unknown` (pure in-memory, zero runtime filesystem access).

## Locale Model

Catalog keys are currently represented by `unic_langid::LanguageIdentifier`: language, optional script,
optional region, and variants. Unicode/private-use extensions are not yet retained as part of the catalog
identity; callers that require extension-aware locale semantics must keep that policy in the host layer until
the dedicated locale negotiation work is completed.

## Entry Points

- `UiMessages`
- `UiTranslator`
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
- `resolve_fluent_message`
- `try_resolve_fluent_message`
- `with_kebab_key`

## Interactions

- Module-owned UI packages use this crate from local `i18n.rs` files via `declare_module_i18n!`.
- Host/runtime code owns effective locale selection; this crate only resolves messages for a supplied locale.
- `@rustok/next-fluent` uses the same Project Fluent bidi-safe default so Rust and Next.js rendering agree on interpolation safety.

## Boundary Rules

- Do not add Leptos, Dioxus, Axum, GraphQL, cookie, header, query, or routing dependencies.
- Do not select the user's locale here; consume the host-provided effective locale.
- Do not add runtime filesystem scanning or environment lookups in production paths.
- Do not add module-specific message keys or business copy to this crate.

## Docs

- [Crate docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
- [Module UI package implementation guide](../../docs/UI/module-package-implementation.md)
