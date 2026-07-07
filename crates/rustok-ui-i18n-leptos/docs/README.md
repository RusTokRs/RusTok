# `rustok-ui-i18n-leptos` Documentation

This crate provides the shared Leptos adapter for `rustok-ui-i18n`.

## Responsibilities

- Store static locale bundles for a Leptos UI package.
- Lazily build a shared `UiMessageCatalog`.
- Resolve messages for an explicit locale through `t_for_locale`.
- Resolve messages from host-provided `UiRouteContext.locale` through `t_from_context`.

## Non-Responsibilities

- Locale negotiation from cookies, headers, query parameters or browser storage.
- Dioxus integration.
- Module-specific translations.

## Verification

- `cargo test -p rustok-ui-i18n-leptos --lib`
