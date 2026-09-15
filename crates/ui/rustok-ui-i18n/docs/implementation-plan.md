# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` owns framework-agnostic `UiMessages` supporting Project
Fluent (`.ftl`) message catalogs, canonical locale normalization
(`normalize_admin_locale`), thread-safe concurrent bundle storage (`Send + Sync`),
zero-allocation hot-path key resolution (`with_kebab_key`), macro declaration
(`declare_module_i18n!`), and fallback resolution. All legacy JSON catalog code
and `serde_json` dependencies have been completely removed in accordance with
the platform Zero-Legacy policy (`AGENTS.md`).

## FFA/FBA boundary

- FFA status: `active`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This crate owns neither framework hooks nor module business copy, routing,
  transport, or locale selection policy. It is strictly framework-agnostic.

## Open results

1. **Adopt Project Fluent (`fluent-bundle`) for message catalogs and grammar.** (Completed)
   Translation catalogs support standard `.ftl` (Fluent Translation List)
   format, isolating natural language grammar, plurals, and parameter interpolation
   without leaking language logic into Rust UI components. Unit tests verify
   Russian plurals (1/2/5) and parameter formatting.

2. **Integrate ICU4X for e-commerce formatting.**
   Provide locale-aware formatting (`icu_decimal`, `icu_datetime`) exposed to Fluent
   messages for locale-aware currency and date rendering.
   **Depends on:** adding relevant `icu4x` modules.
   **Verification:** round-trip tests with complex BCP 47 tags and currency formatting across locales.

3. **Unified framework-agnostic `UiMessages` facade & declare macro.** (Completed)
   `UiMessages` provides thread-safe concurrent bundle management, `t_for_locale`,
   `format` with `FluentArgs`, `fluent_args!` helper macro, `declare_module_i18n!`
   macro, and canonical `normalize_admin_locale`.

4. **Zero-legacy JSON catalog elimination & zero-allocation hot path.** (Completed)
   All JSON catalog types, methods, and `serde_json` were deleted. Hot-path key
   conversion from dotted syntax to kebab-case uses a zero-allocation stack buffer.

5. **Atomic cutover across all UI modules.** (Completed)
   All 43+ module `i18n.rs` files migrated to `rustok_ui_i18n::UiMessages` via `declare_module_i18n!`.
   All module `Cargo.toml` files updated to declare `rustok-ui-i18n.workspace = true`.

## Verification

- `cargo test -p rustok-ui-i18n --lib`
- `cargo check -p rustok-brand-admin`
- `cargo check -p rustok-cart-storefront`

## Change rules

1. Keep all Leptos, Dioxus, and host framework dependencies out of this crate.
2. Keep locale selection with the host/runtime effective-locale contract (`UiRouteContext.locale`).
3. Domain modules own their own `.ftl` / `.json` message files; this crate provides the engine and shared formatting helpers.
4. Follow the Zero-Legacy policy: no compatibility stubs or deprecated aliases.
