# rustok-ui-i18n

## Purpose

`rustok-ui-i18n` provides framework-agnostic UI message catalog helpers for
RusToK module-owned UI packages and future UI adapters.

## Responsibilities

- Manage Project Fluent (`.ftl`) message catalogs with natural grammar and pluralization.
- Provide thread-safe concurrent bundle management via `UiMessages` (`Send + Sync`).
- Resolve message keys from the host-provided effective locale with zero-allocation stack buffering.
- Apply the platform UI fallback chain without depending on Leptos, Dioxus, Next.js, or host routing.
- Provide canonical locale normalization (`normalize_admin_locale`).
- Provide boilerplate reduction macro (`declare_module_i18n!`) for module UI packages.
- Keep UI i18n catalog logic out of `rustok-api` and framework-specific crates.

## Entry Points

- `UiMessages`
- `declare_module_i18n!`
- `fluent_args!`
- `t!`
- `module_t!`
- `normalize_admin_locale`
- `build_fluent_bundle`
- `build_fluent_catalog`
- `resolve_fluent_message`
- `with_kebab_key`

## Interactions

- Module-owned UI packages use this crate from local `i18n.rs` files via `UiMessages`.
- Host/runtime code still owns effective locale selection; this crate only resolves messages for a supplied locale.

## Boundary Rules

- Do not add Leptos, Dioxus, Axum, GraphQL, cookie, header, query, or routing dependencies.
- Do not select the user's locale here; consume the host-provided effective locale.
- Do not add module-specific message keys or business copy to this crate.

## Docs

- [Platform docs index](../../docs/index.md)
- [Module UI package implementation guide](../../docs/UI/module-package-implementation.md)
