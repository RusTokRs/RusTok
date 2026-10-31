# rustok-ui-i18n / CRATE_API

## Purpose

`rustok-ui-i18n` provides framework-agnostic Project Fluent message catalogs, BCP-47 / Unicode locale normalization, CLDR-compliant fallback chains, bounded `Accept-Language` header parsing, and thread-safe UI message translation helpers for RusToK.

## Architecture & Responsibilities

- **Thread Safety & WASM Portability**: `UiMessages` and `PreparedUiMessages` implement `Send + Sync` and compile without filesystem access for WebAssembly targets.
- **Project Fluent Message Catalogs**: Wraps `fluent-bundle` into ergonomic catalog abstractions with lazy (`LazyUiMessages`) and pre-validated (`PreparedUiMessages`) runtime modes.
- **Bidirectional Unicode Isolation**: Preserves Unicode directional isolation characters (`\u{2068}` and `\u{2069}`) to prevent text injection and bidirectional formatting bleeding.
- **Fail-Soft Locale Fallback Hierarchy**: Implements CLDR candidate resolution: exact locale $\to$ language without variant $\to$ region $\to$ script $\to$ configured default $\to$ `"en"` $\to$ message key fallback.
- **Header Parsing & Protection**: Parses and weights HTTP `Accept-Language` headers with bounded length and range limits to prevent algorithmic denial of service.

## Public Surface

### Modules
- `accept_language`: `Accept-Language` header parsing, q-factor sorting, and locale preference resolution.
- `bundle`: Project Fluent catalog and bundle builders (`build_fluent_bundle`, `build_fluent_catalog`).
- `error`: Typed error structures (`BundleBuildError`, `I18nError`, `MessageKeyError`, `AcceptLanguageError`).
- `locale`: Unicode locale normalization, canonicalization, candidate chains, and text direction (`TextDirection`).
- `messages`: Thread-safe runtime catalog (`UiMessages`), prepared translator (`UiLocaleTranslator`), and message resolution.
- `lazy`: On-demand parser and catalog facade (`LazyUiMessages`).
- `macros`: Diagnostic and lookup macros (`declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`).
- `prelude`: High-level consumer prelude.

### Primary Public Types & Enums
- `UiMessages`: Primary thread-safe container for multi-locale Fluent resources.
- `PreparedUiMessages`: Compiled and schema-verified catalog ready for concurrent lookup.
- `UiLocaleTranslator`: Bound translator instance for a specific resolved locale.
- `ResolvedMessage`: Translated string with source-locale metadata and fallback indicators.
- `TextDirection`: Directionality enum (`Ltr`, `Rtl`).
- `FluentCatalog`: Compiled fluent catalog map.
- `AcceptLanguagePreference`: Structured header language preference entry.
- `I18nError`: Comprehensive translation and resolution error type.

### Methods & Helpers
- `locale_candidates(requested: Option<&str>, default_locale: &str) -> Vec<String>`: Calculates fallback search chain.
- `normalize_locale_tag(tag: &str) -> Option<String>`: Normalizes locale strings into BCP-47 canonical tags.
- `normalize_admin_locale(locale: Option<&str>) -> &'static str`: Maps admin dashboard locale to supported codes.
- `parse_accept_language(header: &str) -> Vec<AcceptLanguagePreference>`: Parses and sorts `Accept-Language` header.
- `preferred_locale_from_accept_language(header: &str, supported: &[&str], default_locale: &str) -> String`: Resolves best matching locale.

## Invariants & Guarantees

1. **Thread Safety**: All public message catalog holders implement `Send + Sync`.
2. **Zero DOM or Framework Bindings**: No dependencies on Leptos, Dioxus, React, or browser global DOM objects.
3. **Bounded Header Processing**: `Accept-Language` header parsing is bounded by `MAX_ACCEPT_LANGUAGE_LEN` (1024 bytes) and `MAX_ACCEPT_LANGUAGE_RANGES` (32 ranges).
4. **BiDi Isolation**: Formatted variables are wrapped in bidirectional isolate marks preventing script direction tampering.
5. **Deterministic Fallback**: Missing keys never panic; they return the key itself wrapped as a fallback in lenient mode or a typed `I18nError` in strict mode.

## Dependencies & Boundaries

- **Upstream Dependencies**:
  - `fluent-bundle`, `fluent-syntax`: Mozilla Project Fluent parsing and runtime formatting.
  - `icu_locale`: Unicode CLDR locale canonicalization and parsing.
  - `unic-langid`: Language identifier handling.
  - `tracing`: Structured logging and diagnostic spans.
- **Downstream Consumers**:
  - `leptos-ui`, `rustok-ui-leptos`, `rustok-ui-dioxus`.
  - Admin dashboards, storefront applications, and module UI packages.
- **Boundary Rules**:
  - UI framework adapters (Leptos/Dioxus) provide reactive signal wrappers over `UiLocaleTranslator`.
  - Database persistence and backend localization are owned by `rustok-i18n`.
