# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` owns framework-agnostic `UiMessages` supporting both Project
Fluent (`.ftl`) and JSON message catalogs, canonical locale normalization
(`normalize_admin_locale`), thread-safe concurrent bundle storage (`Send + Sync`),
and fallback resolution. All 43+ UI module packages use `UiMessages` directly,
and the redundant `rustok-ui-i18n-leptos` adapter crate has been eliminated in
accordance with the platform Zero-Legacy policy.

## FFA/FBA boundary

- FFA status: `active`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This crate owns neither framework hooks nor module business copy, routing,
  transport, or locale selection policy. It is strictly framework-agnostic.

## Open results

1. **Adopt Project Fluent (`fluent-bundle`) for message catalogs and grammar.** (Completed)
   Translation catalogs support standard `.ftl` (Fluent Translation List)
   format alongside JSON bundles, isolating natural language grammar, plurals, and
   parameter interpolation without leaking language logic into Rust UI components.
   Unit tests verify Russian plurals (1/2/5) and parameter formatting.

2. **Integrate ICU4X for e-commerce formatting.**
   Provide locale-aware formatting (`icu_decimal`, `icu_datetime`) exposed to Fluent
   messages for locale-aware currency and date rendering.
   **Depends on:** adding relevant `icu4x` modules.
   **Verification:** round-trip tests with complex BCP 47 tags and currency formatting across locales.

3. **Unified framework-agnostic `UiMessages` facade.** (Completed)
   `UiMessages` provides thread-safe static bundle management, `t_for_locale`,
   `format` with `FluentArgs`, `fluent_args!` helper macro, and canonical
   `normalize_admin_locale`.

4. **Atomic cutover across all UI modules.** (Completed)
   All 43+ module `i18n.rs` files migrated to `rustok_ui_i18n::UiMessages`.
   All module `Cargo.toml` files updated to declare `rustok-ui-i18n.workspace = true`.

5. **Elimination of `rustok-ui-i18n-leptos`.** (Completed)
   `rustok-ui-i18n-leptos` was removed atomically. Because `UiMessages` operates
   purely on framework-agnostic Rust types and host-supplied effective locales,
   separate framework adapter crates are unnecessary.

## Verification

- `cargo test -p rustok-ui-i18n --lib`
- `cargo check -p rustok-brand-admin`
- `cargo check -p rustok-cart-storefront`

## Change rules

1. Keep all Leptos, Dioxus, and host framework dependencies out of this crate.
2. Keep locale selection with the host/runtime effective-locale contract (`UiRouteContext.locale`).
3. Domain modules own their own `.ftl` / `.json` message files; this crate provides the engine and shared formatting helpers.
4. Follow the Zero-Legacy policy: no compatibility stubs or deprecated aliases.
