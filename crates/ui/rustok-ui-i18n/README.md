# rustok-ui-i18n

## Purpose

`rustok-ui-i18n` provides framework-agnostic UI message catalog helpers for
RusToK module-owned UI packages and future UI adapters.

## Architecture & Modules

The crate is structured into focused domain modules:

- `locale`: complete Unicode locale normalization (`normalize_unicode_locale`), ICU4X/CLDR alias canonicalization, Fluent catalog-identity projection (`normalize_locale_tag`), locale directionality (`locale_text_direction`), canonical admin locale resolution (`normalize_admin_locale`), and structured fallback candidate chains (`locale_candidates`).
- `bundle`: Concurrent Project Fluent (`.ftl`) bundle (`build_fluent_bundle`) and catalog (`build_fluent_catalog`, `try_build_fluent_catalog`, `bundle::build_fluent_catalog_report`, `FluentCatalog`) construction with Unicode bidi isolation enabled for interpolated values.
- `messages`: Core thread-safe UI message facade (`UiMessages`), fail-closed prepared runtime (`PreparedUiMessages`), borrowed translator (`UiTranslator`), prepared per-locale translator (`UiLocaleTranslator`), source-locale provenance (`ResolvedMessage`), compound-message attribute lookup, transitive message/term schema validation, stack-buffered safe kebab-case key conversion (`with_kebab_key`), strict/lenient candidate resolution, and cached lazy-initialization diagnostics.
- `lazy`: Opt-in per-locale Fluent parsing (`LazyUiMessages`, `LazyUiLocaleTranslator`) for large embedded locale sets while preserving the same fallback, formatting, provenance, and validation contracts.
- `error`: Typed errors (`BundleBuildError`, `I18nError`) for locale, catalog, lookup, and formatting failures. Both public error enums are non-exhaustive; downstream matches must retain a wildcard arm so new diagnostics can be added compatibly.
- `macros`: Ergonomic macros (`declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`).

## Responsibilities

- Manage Project Fluent (`.ftl`) message catalogs with natural grammar, selectors, pluralization, and parameter interpolation.
- Provide thread-safe concurrent bundle management via `UiMessages` (`Send + Sync`).
- Preserve Project Fluent Unicode directional isolation around interpolated values so mixed LTR/RTL UI text renders safely.
- Resolve message values and attributes from the host-provided effective locale with stack buffering for dotted keys up to 128 bytes.
- Return the canonical source locale on provenance-aware lookup paths, so callers can observe parent/default fallback without duplicating resolver logic.
- Validate message IDs against Fluent's ASCII identifier grammar (with documented dotted aliases) and reject Unicode control characters before lookup.
- Accept arbitrary well-formed Unicode locale requests, canonicalize deprecated CLDR aliases, and safely project formatting/private-use extensions onto extension-free Fluent catalog identities.
- Apply a structured platform UI fallback chain (exact locale -> variants removed as one layer -> CLDR-inferred script for region-disambiguated languages -> region -> script -> language -> default locale -> `"en"` -> fallback string) without depending on Leptos, Dioxus, Next.js, or host routing.
- Expose CLDR writing direction instead of forcing hosts to maintain incomplete RTL language lists.
- Provide strict startup preparation (`UiMessages::prepare`) and fail-soft initialization diagnostics without rebuilding the lazy catalog.
- Keep request/catalog/default locale normalization bounded on raw input before trim/normalization work.
- Keep UI i18n catalog logic out of `rustok-api` and framework-specific crates.
- Maintain compile-time portability for `wasm32-unknown-unknown` (pure in-memory, zero runtime filesystem access).

## Locale Model

Runtime requests are parsed as complete Unicode locale identifiers with ICU4X. This accepts language,
script, region, variants, Unicode (`-u-`), transformed (`-t-`), and private-use (`-x-`) extensions and
canonicalizes deprecated aliases with CLDR data. `normalize_unicode_locale` preserves that complete
identity for host date/number/calendar/collation services. `normalize_locale_tag` deliberately projects
it to the extension-free language identifier required by Fluent 0.16 catalog lookup.

Catalog declarations and the configured default remain extension-free: formatting preferences must not
create duplicate message catalogs. They are canonicalized through the same alias data, so legacy and
modern spellings cannot silently become separate identities. Region-only requests whose region changes
the language's likely writing system receive a CLDR-backed script branch, for example
`zh-TW-u-ca-chinese -> zh-TW -> zh-Hant-TW -> zh-Hant -> zh`.

The engine is not limited to English and Russian. The no-argument module macro remains a compatibility
shortcut for the current two-file modules; new multilingual modules can declare any checked-in set with
`declare_module_i18n!(default = "en", locales = ["en", "ar", "de", "ja", "zh-Hant"]);`.
The library supplies locale mechanics and CLDR plural selection, not translated product copy: every
advertised locale still needs an owned, reviewed `.ftl` catalog.

Large embedded catalogs can opt into `LazyUiMessages` directly or use
`declare_module_i18n!(lazy, default = "en", locales = [...])`. Locale declarations are canonicalized
and indexed together, while each concurrent Fluent bundle is parsed only when its locale first appears
in a lookup fallback chain. Failed bundles are cached, diagnosed once, and skipped using the same
fail-soft fallback semantics as `UiMessages`; `validate()` and `prepare()` remain complete fail-closed
checks. This reduces startup parsing and resident bundle state, but does **not** remove `include_str!`
bytes from native/WASM binaries. Truly downloadable catalogs require a host-owned storage adapter.

The Rust locale-input policy rejects raw locale strings longer than 64 bytes before trimming or
underscore normalization. Runtime lookup, direct bundle construction, strict/lenient catalog
construction, and default-locale validation use the same bounded-input contract. Oversized diagnostics
retain only length metadata and never copy or log the full untrusted payload.

## Key Identity and Catalog Collisions

Application-facing dotted keys are a convenience spelling for Fluent kebab IDs: `account.profile.title`
and `account-profile-title` intentionally resolve to the same message identity. Treat those spellings as aliases,
not as two independent keys; a module must not assign different semantics to the dotted and kebab forms.

Catalog construction has three explicit modes:

- `try_build_fluent_catalog` is fail-closed: malformed/oversized locale input, invalid resources, and duplicate normalized locale keys are returned as typed errors.
- `build_fluent_catalog` is the pure lenient rendering convenience path: invalid catalog entries are skipped, while the first **parseable locale input** reserves each normalized locale identity before its FTL payload is parsed. Use the report API for diagnostics; `UiMessages` logs its cached report once. A malformed first payload therefore cannot be silently replaced by a later duplicate; the later duplicate is diagnosed and skipped.
- `bundle::build_fluent_catalog_report` uses the same lenient behavior but additionally returns typed diagnostics for every skipped entry so startup health code does not need to scrape logs. In a malformed-first collision this preserves both diagnostics in input order: the original bundle failure followed by `DuplicateLocale` for the shadowing entry.

`UiMessages` caches the lenient report once. `initialization_diagnostics()` exposes both skipped-entry
errors and invalid/oversized/missing default-locale configuration from that same initialization; it does
not rebuild the catalog solely to recover diagnostics.

Within an individual Fluent resource, message/resource conflicts are handled by Project Fluent's
`add_resource` validation and surface as `BundleBuildError::AddResource`; this crate does not silently invent
an override policy for Rust catalogs.

## Entry Points

- `UiMessages`
- `LazyUiMessages`
- `LazyUiLocaleTranslator`
- `PreparedUiMessages`
- `UiTranslator`
- `UiLocaleTranslator`
- `ResolvedMessage`
- `BundleBuildError`
- `I18nError`
- `declare_module_i18n!`
- `fluent_args!`
- `t!`
- `module_t!`
- `normalize_admin_locale`
- `normalize_unicode_locale`
- `normalize_locale_tag`
- `locale_text_direction` / `TextDirection`
- `locale_candidates`
- `build_fluent_bundle`
- `build_fluent_catalog`
- `try_build_fluent_catalog`
- `bundle::build_fluent_catalog_report`
- `bundle::FluentCatalogBuildReport`
- `resolve_fluent_message` / `resolve_fluent_message_with_locale`
- `try_resolve_fluent_message` / `try_resolve_fluent_message_with_locale`
- `resolve_fluent_attribute` / `try_resolve_fluent_attribute`
- `with_kebab_key`
- `MessageSchema` / `MessageEntrySchema`
- `extract_locale_schemas` / `extract_locale_entry_schemas`
- `validate_catalog_schemas`

Schema extraction validates and normalizes its locale argument through the same
64-byte-bounded locale parser used by catalog construction. Rich entry schemas keep
message values and attributes separate, resolve message/term references transitively,
account for named term arguments, and reject missing/cyclic references. Cross-locale
validation rejects duplicate normalized locale identities and normalizes the configured
default locale before comparing values and attributes with default-locale contracts.

## Interactions

- Module-owned UI packages use this crate from local `i18n.rs` files via `declare_module_i18n!`; large locale sets may choose its explicit `lazy` form.
- Host/runtime code owns effective locale selection, including `Accept-Language` parsing and tenant/user policy; this crate only resolves messages for a supplied locale.
- `@rustok/next-fluent` uses the same Project Fluent bidi-safe default and bounded locale-input policy. Extension-aware canonicalization and CLDR likely-script fallback are now the reference behavior that the separately versioned Next adapter must mirror.

## Boundary Rules

- Do not add Leptos, Dioxus, Axum, GraphQL, cookie, header, query, or routing dependencies.
- Do not select the user's locale here; consume the host-provided effective locale.
- Do not add runtime filesystem scanning or environment lookups in production paths.
- Do not add module-specific message keys or business copy to this crate.
- Treat exported dependency types and helper functions as compatibility surface until an explicit migration window narrows them; prefer additive facade APIs over silent removals.

## Docs

- [Crate docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Engineering audit (2026-09-27)](./docs/engineering-audit-2026-09-27.md)
- [Platform docs index](../../../docs/index.md)
- [Module UI package implementation guide](../../../docs/UI/module-package-implementation.md)
