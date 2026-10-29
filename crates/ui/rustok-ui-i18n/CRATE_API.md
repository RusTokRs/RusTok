# rustok-ui-i18n / CRATE_API

## Public surface

### Modules
- `accept_language`: bounded `Accept-Language` header parsing, thousandth q-value weighting, and preference projection.
- `bundle`: Project Fluent (`.ftl`) bundle and catalog builders (`build_fluent_bundle`, `build_fluent_catalog`, `FluentCatalog`).
- `error`: typed error definitions (`BundleBuildError`, `I18nError`, `MessageKeyError`, `AcceptLanguageError`).
- `locale`: Unicode locale normalization (`normalize_unicode_locale`), CLDR canonicalization, fallback chains (`locale_candidates`), and writing direction (`locale_text_direction`).
- `messages`: thread-safe UI message facade (`UiMessages`), prepared translator (`UiLocaleTranslator`), schema verification, and resolution functions.
- `lazy`: lazy on-demand locale parsing (`LazyUiMessages`, `LazyUiLocaleTranslator`).
- `macros`: ergonomic macros (`declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`).
- `prelude`: canonical import prelude for UI consumers.

### Primary Public Types & Enums
- `UiMessages`: thread-safe (`Send + Sync`) Project Fluent catalog holder.
- `PreparedUiMessages`: pre-validated runtime bundle set.
- `UiLocaleTranslator`: prepared translator bound to a specific resolved locale.
- `ResolvedMessage`: translated message with source-locale provenance.
- `TextDirection`: text flow direction (`Ltr`, `Rtl`).
- `FluentCatalog`: compiled Project Fluent catalog.
- `AcceptLanguagePreference`: structured language preference extracted from headers.
- `I18nError`: comprehensive translation and catalog error enum.

## Invariants

- **Thread Safety & WASM Portability**: `UiMessages` must be `Send + Sync` and compile cleanly for `wasm32-unknown-unknown` without runtime filesystem access.
- **Bidirectional Unicode Isolation**: Interpolated parameter values must retain Project Fluent Unicode isolation markers to prevent LTR/RTL text bleeding.
- **Fail-Soft UI Fallback**: Message resolution traverses a structured fallback chain: exact locale $\to$ language without variant $\to$ region $\to$ script $\to$ language $\to$ configured default $\to$ `"en"` $\to$ key fallback string.
- **Framework Independence**: Pure Rust library with zero dependencies on Leptos, Dioxus, Next.js, or browser runtime engines.
