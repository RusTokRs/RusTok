# rustok-ui-i18n

## Purpose

`rustok-ui-i18n` provides framework-agnostic UI message catalog helpers for
RusToK module-owned UI packages and future UI adapters.

## Responsibilities

- Build flat message catalogs from nested JSON locale bundles.
- Resolve message keys from the host-provided effective locale.
- Apply the platform UI fallback chain without depending on Leptos, Dioxus, Next.js, or host routing.
- Keep UI i18n catalog logic out of `rustok-api` and framework-specific crates.

## Entry Points

- `UiMessageCatalog`
- `build_ui_message_catalog`
- `resolve_ui_message`
- `resolve_ui_message_or_fallback`

## Interactions

- Module-owned UI packages use this crate from local `i18n.rs` files.
- Leptos packages use `rustok-ui-i18n-leptos` for shared adapter boilerplate.
- Dioxus packages must use a sibling `rustok-ui-i18n-dioxus` adapter when Dioxus enters the workspace.
- Host/runtime code still owns effective locale selection; this crate only resolves messages for a supplied locale.

## Boundary Rules

- Do not add Leptos, Dioxus, Axum, GraphQL, cookie, header, query, or routing dependencies.
- Do not select the user's locale here; consume the host-provided effective locale.
- Do not add module-specific message keys or business copy to this crate.

## Docs

- [Platform docs index](../../docs/index.md)
- [Module UI package implementation guide](../../docs/UI/module-package-implementation.md)
