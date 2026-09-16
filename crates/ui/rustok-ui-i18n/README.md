# rustok-ui-i18n

## Purpose

`rustok-ui-i18n` provides framework-agnostic UI message catalog helpers for
RusToK module-owned UI packages and future UI adapters.

## Architecture & Modules

The crate is structured into focused domain modules:

- `locale`: BCP 47 language identifier normalization (`normalize_locale_tag`), canonical admin locale resolution (`normalize_admin_locale`), and fallback candidate chains (`locale_candidates`).
- `bundle`: Concurrent Project Fluent (`.ftl`) bundle (`build_fluent_bundle`) and catalog (`build_fluent_catalog`, `FluentCatalog`) construction with zero-isolating string formatting.
- `messages`: Core thread-safe UI message facade (`UiMessages`), borrowed translator (`UiTranslator`), zero-allocation stack-buffered kebab-case key conversion (`with_kebab_key`), and candidate resolution (`resolve_fluent_message`).
- `error`: Typed errors (`BundleBuildError`, `I18nError`) for parse and resource failures.
- `macros`: Ergonomic macros (`declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`).

## Responsibilities

- Manage Project Fluent (`.ftl`) message catalogs with natural grammar, selectors, and pluralization.
- Provide thread-safe concurrent bundle management via `UiMessages` (`Send + Sync`).
- Resolve message keys from the host-provided effective locale with zero-allocation stack buffering for keys <= 128 bytes.
- Apply the platform UI fallback chain (regional tag -> language base -> default locale -> "en" -> fallback string) without depending on Leptos, Dioxus, Next.js, or host routing.
- Provide canonical locale normalization (`normalize_admin_locale`).
- Provide boilerplate reduction macro (`declare_module_i18n!`) for module UI packages.
- Keep UI i18n catalog logic out of `rustok-api` and framework-specific crates.
- Maintain full compile-time portability for `wasm32-unknown-unknown` (pure in-memory, zero runtime filesystem access).

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
- `resolve_fluent_message`
- `with_kebab_key`

## Interactions

- Module-owned UI packages use this crate from local `i18n.rs` files via `declare_module_i18n!`.
- Host/runtime code still owns effective locale selection; this crate only resolves messages for a supplied locale.
- Parity with `@rustok/next-fluent`: shared `.ftl` conventions and zero-isolating string configuration.

## Boundary Rules

- Do not add Leptos, Dioxus, Axum, GraphQL, cookie, header, query, or routing dependencies.
- Do not select the user's locale here; consume the host-provided effective locale.
- Do not add runtime filesystem scanning or environment lookups in production paths.
- Do not add module-specific message keys or business copy to this crate.

## Docs

- [Platform docs index](../../docs/index.md)
- [Module UI package implementation guide](../../docs/UI/module-package-implementation.md)
