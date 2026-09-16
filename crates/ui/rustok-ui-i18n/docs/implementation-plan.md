# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` owns framework-agnostic Project Fluent message catalog resolution supporting
Unicode Language Identifier normalization, thread-safe concurrent bundle storage (`Send + Sync`),
stack-buffered dotted-to-kebab key conversion, module declaration macros, typed diagnostics, and
requested/default/platform fallback resolution. The crate is decomposed into `locale`, `bundle`,
`messages`, `error`, and `macros` modules.

All production resources are compile-time embedded/in-memory. The crate does not select the user's
locale and does not depend on a UI framework, router, HTTP stack, cookies, environment variables or
runtime filesystem discovery.

## FFA/FBA boundary

- FFA status: `active`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This crate owns neither framework hooks nor module business copy, routing, transport, or locale selection policy.

## Completed foundations

1. **Project Fluent catalogs and grammar.**
   `.ftl` resources are parsed into concurrent `FluentBundle`s and support Fluent selectors,
   pluralization and interpolation.

2. **Modular architecture decomposition.**
   - `locale`: locale normalization and candidate chains;
   - `bundle`: Fluent bundle/catalog construction;
   - `messages`: `UiMessages`, `UiTranslator`, strict/lenient resolution and key conversion;
   - `error`: `BundleBuildError` and `I18nError`;
   - `macros`: `declare_module_i18n!`, `fluent_args!`, `t!`, `module_t!`.

3. **Strict and lenient catalog paths.**
   `try_build_fluent_catalog` fails closed on malformed locales, malformed FTL and duplicate
   normalized locales. `build_fluent_catalog` remains a lenient UI path and emits diagnostics for
   skipped invalid input. `UiMessages::validate` gives CI/tests a fail-closed validation hook.

4. **Strict and lenient message resolution.**
   `try_resolve_fluent_message` / `try_format` report missing keys and formatting failures through
   `I18nError`; lenient rendering logs formatting failures and allows the caller's literal fallback.

5. **Bidi-safe Fluent interpolation.**
   Unicode FSI/PDI isolation is enabled by default in both `rustok-ui-i18n` and
   `@rustok/next-fluent`. Contract tests explicitly cover an LTR interpolation inside an RTL Arabic
   message. Existing grammar tests compare visible content after removing isolation markers rather
   than asserting the unsafe absence of those markers.

6. **Thread safety and lazy initialization.**
   `UiMessages` uses `std::sync::OnceLock` and concurrent Fluent bundles. Contract tests assert
   `Send + Sync` and exercise concurrent message resolution from multiple native threads.

7. **WASM-friendly production boundary.**
   Production code is in-memory and has no runtime filesystem dependency. The supported verification
   target remains `wasm32-unknown-unknown`.

8. **Zero-legacy JSON catalog elimination.**
   Legacy JSON catalog code and `serde_json` dependency have been removed from this crate.

9. **Module cutover.**
   Module UI packages use the shared `UiMessages`/macro boundary instead of maintaining separate
   localization engines.

## Remaining engineering work

### 1. Locale model and negotiation

The current catalog key type is `unic_langid::LanguageIdentifier`. It supports language, optional
script, region and variants, but it is not a full extension-preserving BCP-47 locale model. Unicode
extensions/private-use subtags must not silently acquire semantics they do not have.

Next steps:
- define the supported locale-tag contract explicitly;
- add matrix tests for scripts, regions and variants;
- decide whether extension-aware catalog identity is required or whether extensions are intentionally
  host-owned and removed before catalog lookup;
- replace string-suffix fallback construction with a structured negotiation abstraction if full locale
  semantics are required.

### 2. Initialization semantics

`UiMessages::validate` is strict, while the lazily cached production catalog is intentionally built
through the lenient path. The API should make that lifecycle distinction explicit so consumers do not
mistake strict message formatting for strict catalog initialization.

Next steps:
- introduce an explicit validated/prepared catalog construction path suitable for application startup;
- make initialization diagnostics inspectable without relying only on logs;
- keep lenient rendering as an intentional opt-in policy rather than an implicit substitute for validation.

### 3. Allocation and hot-path profile

`with_kebab_key` avoids heap allocation for dotted keys up to 128 bytes, but locale normalization and
candidate creation currently allocate `String`s/`Vec`s per lookup. Optimize only after measuring.

Next steps:
- add Criterion benchmarks for no-args lookup, args formatting, requested-locale fallback and missing keys;
- measure locale parsing/candidate allocation separately from Fluent formatting;
- add a prepared locale/resolver abstraction if profiling shows candidate construction is material;
- benchmark tens of locales and large message catalogs before changing the map/storage representation.

### 4. Public API and semver surface

The crate publicly re-exports Fluent and `unic-langid` types and exposes its internal modules. This is
convenient today but couples downstream semver to public dependencies.

Next steps:
- inventory real workspace consumers before removing or wrapping any public dependency types;
- prefer additive facade APIs first;
- reserve any surface reduction for an explicit breaking-change window.

### 5. Macro contract hardening

The runtime behavior is covered, but exported macros still need consumer-style compile tests.

Next steps:
- compile-pass tests from an external-crate context;
- compile-fail diagnostics for invalid macro forms;
- verify renamed crate usage and hygiene assumptions around module-local `MESSAGES`.

### 6. Catalog integrity and collision policy

Dotted keys are mapped to kebab keys (`a.b` -> `a-b`), so source-level aliases can collide by design.
The library must either document that equivalence as a key convention or validate catalogs against
ambiguous aliases.

Next steps:
- add key convention/collision fixtures;
- add duplicate-message and malformed-resource matrices;
- define first-wins/strict-error behavior for every catalog construction path.

### 7. Locale-aware domain formatting

Currency/date/number formatting should remain a separate, measured capability rather than be mixed
into core lookup semantics. If introduced, expose it through Fluent functions and keep framework
concerns outside this crate.

## Verification matrix

Rust foundation:
- `cargo test -p rustok-ui-i18n`
- `cargo clippy -p rustok-ui-i18n -- -D warnings`
- `cargo check -p rustok-ui-i18n --target wasm32-unknown-unknown`

Next.js Fluent parity surface:
- `cd packages/next-fluent && npm run verify`

Follow-up test work:
- property tests for normalization/candidate invariants and dotted-key conversion;
- malformed FTL and duplicate-key fixtures;
- compile tests for exported macros/public API;
- fuzz targets for FTL/catalog ingestion boundaries where project policy allows fuzz infrastructure;
- concurrency tests that race first initialization as well as steady-state lookup;
- locale/fallback matrices covering language/script/region/variant combinations;
- benchmarks for small and large catalog shapes.

## Change rules

1. Keep Leptos, Dioxus, Axum, GraphQL, cookie, header, query and routing dependencies out of this crate.
2. Keep locale selection with the host/runtime effective-locale contract.
3. Domain modules own their `.ftl` message files; this crate owns the engine and shared formatting boundary.
4. Do not silently weaken Unicode bidi safety for prettier serialized strings.
5. Do not claim full BCP-47 extension semantics until the locale representation actually preserves them.
6. Make performance changes from benchmark evidence rather than replacing simple structures speculatively.
7. Avoid breaking public API until workspace consumers have been inventoried and migrated.
