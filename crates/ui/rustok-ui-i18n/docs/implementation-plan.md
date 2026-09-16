# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` owns framework-agnostic UI message catalog resolution supporting
Project Fluent (`.ftl`) message catalogs, canonical locale normalization
(`normalize_admin_locale`), thread-safe concurrent bundle storage (`Send + Sync`),
zero-allocation hot-path key resolution (`with_kebab_key`), macro declaration
(`declare_module_i18n!`), typed error diagnostics (`BundleBuildError`), and fallback resolution.
The crate is modularized into `locale`, `bundle`, `messages`, `error`, and `macros` submodules.
All legacy JSON catalog code and `serde_json` dependencies have been completely removed in accordance with
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

2. **Modular architecture decomposition.** (Completed)
   Refactored monolithic `lib.rs` into clear submodules:
   - `locale`: BCP 47 locale normalization and candidate chains.
   - `bundle`: Concurrent Fluent bundle and catalog compilation.
   - `messages`: `UiMessages` and `UiTranslator` facades, zero-allocation kebab-case hot path.
   - `error`: Typed `BundleBuildError` and `I18nError`.
   - `macros`: `declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`.

3. **Typed error hierarchy.** (Completed)
   Replaced raw string errors in `build_fluent_bundle` with `BundleBuildError` (`InvalidLocale`,
   `FluentParse`, `AddResource`) implementing `Display` and `std::error::Error`.

4. **Formalized contract & invariant tests.** (Completed)
   Added `tests/contract_tests.rs` covering:
   - Full Russian cardinal pluralization matrix (1, 2, 3, 4, 5, 6, 10, 11, 12, 14, 19, 20, 21, 22, 24, 25, 101, 102, 105, 111).
   - English pluralization matrix (0, 1, 2, 5, 21).
   - Multi-bundle fallback chain (`ru-RU` -> `ru` -> `en` -> fallback string).
   - Zero-isolating string formatting (guaranteeing no `\u{2068}` or `\u{2069}` Unicode directional markers, matching `@rustok/next-fluent`).
   - Thread safety: compile-time `Send + Sync` assertions and multi-threaded concurrent resolution.

5. **WASM portability check.** (Completed)
   Verified clean compilation for `wasm32-unknown-unknown` with zero warnings. The crate operates
   100% in-memory with compile-time embedded resources, with zero runtime filesystem or environment dependencies.

6. **Integrate ICU4X for e-commerce formatting.**
   Provide locale-aware formatting (`icu_decimal`, `icu_datetime`) exposed to Fluent
   messages for locale-aware currency and date rendering.
   **Depends on:** adding relevant `icu4x` modules.
   **Verification:** round-trip tests with complex BCP 47 tags and currency formatting across locales.

7. **Unified framework-agnostic `UiMessages` facade & declare macro.** (Completed)
   `UiMessages` provides thread-safe concurrent bundle management, `t_for_locale`,
   `format` with `FluentArgs`, `fluent_args!` helper macro, `declare_module_i18n!`
   macro, and canonical `normalize_admin_locale`.

8. **Zero-legacy JSON catalog elimination & zero-allocation hot path.** (Completed)
   All JSON catalog types, methods, and `serde_json` were deleted. Hot-path key
   conversion from dotted syntax to kebab-case uses a zero-allocation stack buffer for keys <= 128 bytes.

9. **Atomic cutover across all UI modules.** (Completed)
   All 43+ module `i18n.rs` files migrated to `rustok_ui_i18n::UiMessages` via `declare_module_i18n!`.
   All module `Cargo.toml` files updated to declare `rustok-ui-i18n.workspace = true`.

## Deep Research Audit Fact-Check & Resolution

An external audit report (`deep-research-report (1).md`) suggested potential P0 issues regarding
global mutable roots, SSR locale leaks, runtime filesystem discovery, and WASM incompatibility.
Verification against the codebase confirmed:
- The crate has never possessed global mutable locale state (`set_current_locale` or `GENERATED_I18N_ROOT`).
  All resolution is stateless and receives `locale: Option<&str>` explicitly per invocation.
- Production runtime has zero filesystem calls (`std::fs` is only used in unit tests to validate module `.ftl` files).
- The crate is purely in-memory and compiles cleanly for `wasm32-unknown-unknown`.
- Arguments use native `FluentArgs` via `fluent_args!`, not `serde_json`.

The valid architectural recommendations from the report (modularization, typed errors, contract tests)
were incorporated into the crate design.

## Verification

- `cargo test -p rustok-ui-i18n` (18 unit and contract tests)
- `cargo check -p rustok-ui-i18n --target wasm32-unknown-unknown`
- `cargo clippy -p rustok-ui-i18n -- -D warnings`
- `cargo test -p rustok-forum-admin --lib`

## Change rules

1. Keep all Leptos, Dioxus, and host framework dependencies out of this crate.
2. Keep locale selection with the host/runtime effective-locale contract (`UiRouteContext.locale`).
3. Domain modules own their own `.ftl` message files; this crate provides the engine and shared formatting helpers.
4. Follow the Zero-Legacy policy: no compatibility stubs or deprecated aliases.
