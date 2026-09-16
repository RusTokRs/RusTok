# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` is the framework-agnostic Project Fluent foundation for module-owned UI copy. The
crate owns catalog construction, bounded locale normalization, requested/default/platform fallback,
strict and lenient resolution, prepared startup/runtime facades, module macros, typed diagnostics,
concurrent bundle storage and benchmark/test infrastructure.

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
   `.ftl` resources are parsed into concurrent `FluentBundle`s with Fluent selectors,
   pluralization and interpolation.

2. **Bidi-safe interpolation.**
   Fluent Unicode FSI/PDI isolation is enabled by default in Rust and `@rustok/next-fluent`, with
   RTL/LTR regression coverage and an explicit low-level opt-out only on the Next implementation.

3. **Strict catalog construction.**
   `try_build_fluent_catalog` fails closed on malformed/oversized locale input, malformed FTL,
   duplicate normalized locales and resource-add failures.

4. **Lenient catalog construction with inspectable diagnostics.**
   `build_fluent_catalog` keeps fail-soft first-wins rendering semantics and tracing diagnostics.
   `bundle::build_fluent_catalog_report` returns the same usable catalog plus typed diagnostics for
   every skipped entry in input order.

5. **Bounded locale-input contract.**
   Runtime lookup and catalog construction share the same 64-byte pre-allocation bound. Oversized
   locale payloads are rejected without copying or logging the full untrusted input. Catalog builders
   parse/canonicalize each locale once.

6. **Prepared fail-closed runtime.**
   `UiMessages::prepare` validates once and returns `PreparedUiMessages` backed by the exact strict
   catalog that passed construction. Strict startup requires the normalized default locale to have an
   exact catalog entry.

7. **Prepared per-locale resolution.**
   `UiLocaleTranslator` precomputes one locale fallback chain per request/render scope and reuses it
   across repeated message lookups, avoiding per-key locale parsing/candidate allocation.

8. **Strict and lenient message resolution.**
   `try_resolve_fluent_message` / strict format APIs report missing keys and Fluent formatting errors;
   lenient rendering returns the caller's explicit literal fallback and never exposes partial malformed output.

9. **Catalog/key collision contract.**
   Dotted application keys and kebab Fluent IDs are explicitly one logical identity (`a.b` == `a-b`).
   Duplicate normalized locales are strict errors and lenient first-wins. Duplicate message IDs are
   rejected by Fluent resource validation.

10. **Macro/public API contract coverage.**
    Exported macros have external-consumer rustdoc compile-pass/compile-fail coverage, including renamed
    crate `$crate` hygiene. Prepared runtime types are exposed from the crate root.

11. **Property and concurrency coverage.**
    Property tests cover locale normalization/candidate invariants and optimized dotted-key conversion.
    Native concurrency tests race first `OnceLock` initialization and steady-state Fluent plural lookup.

12. **Performance measurement infrastructure.**
    Criterion covers locale-candidate/prepared construction, direct/default/missing/interpolated lookup,
    22-locale catalog shape and 1,000-message catalog shape. Catalog construction no longer reparses each locale.

13. **Focused verification.**
    Path-filtered Rust and Next Fluent workflows provide package-level signals independent from unrelated
    monorepo baseline failures. Rust verification includes format, tests/doctests, Clippy and WASM check.

14. **WASM-friendly production boundary.**
    Production code is in-memory and has no runtime filesystem dependency; `wasm32-unknown-unknown`
    remains a supported compile target.

## Remaining engineering work

### 1. Locale model and extension semantics

The catalog key type remains `unic_langid::LanguageIdentifier`. It models language, optional script,
region and variants, but not Unicode/private-use extensions.

Remaining decisions:
- decide whether extension-aware catalog identity is ever required;
- otherwise make extension stripping/selection explicitly host-owned and keep this crate intentionally
  `LanguageIdentifier`-only;
- if the representation is widened later, replace suffix-string fallback construction with a structured
  locale-negotiation abstraction at the same time rather than layering partial extension semantics on top.

### 2. Lazy `UiMessages` initialization diagnostics

The low-level lenient catalog report is now inspectable, but `UiMessages::fluent_catalog()` still caches
only the resulting `FluentCatalog` inside `OnceLock`; callers using that convenience path cannot query the
skipped-entry report after first initialization.

Next slice:
- decide whether `UiMessages` should cache a report object or expose an explicit lenient-initialization API;
- preserve `fluent_catalog() -> &FluentCatalog` compatibility;
- avoid duplicating catalog construction solely to recover diagnostics.

### 3. Remove the remaining unnecessary `unsafe`

`with_kebab_key` uses `from_utf8_unchecked` after replacing only ASCII `.` bytes with ASCII `-`. The
invariant is currently property-tested, but the optimization is not important enough to keep `unsafe`
without measured evidence that safe validation is material.

Next slice:
- benchmark a safe stack-buffer conversion;
- prefer a safe implementation if impact is negligible;
- retain the heap fallback for long keys.

### 4. Public API / semver surface before 1.0

The crate is currently workspace version `0.1.0` and publicly exposes modules, Fluent types,
`unic_langid::LanguageIdentifier`, `FluentCatalog` internals and public error enums.

Before a stable release:
- inventory real workspace/external consumers;
- decide which dependency types are intentional public contract versus implementation leakage;
- decide whether error enums should become non-exhaustive before downstream exhaustive matches harden;
- prefer additive facade APIs and reserve removals/type wrapping for an explicit migration window.

### 5. Fuzz and stress validation

Existing property/malformed/concurrency tests are broad but deterministic test suites are not a substitute
for adversarial parser/runtime stress.

Next steps:
- add fuzz targets for locale normalization/candidate construction and FTL/catalog ingestion if repository
  fuzz infrastructure is approved;
- add bounded stress fixtures for very large message catalogs and malformed-resource batches;
- verify diagnostic collection does not amplify attacker-controlled input sizes.

### 6. Retained benchmark evidence

The benchmark matrix exists, but optimization decisions should retain actual before/after numbers when a
hot-path change is proposed. Do not replace `BTreeMap`, Fluent storage, or candidate representation based
on intuition alone.

### 7. Locale-aware domain formatting

Currency/date/number formatting remains intentionally outside core lookup semantics. If introduced,
expose it through Fluent functions and keep framework/transport concerns outside this crate.

## Verification matrix

Rust foundation:
- `cargo fmt -p rustok-ui-i18n -- --check`
- `cargo test -p rustok-ui-i18n --all-features`
- `cargo clippy -p rustok-ui-i18n --all-targets --all-features -- -D warnings`
- `cargo check -p rustok-ui-i18n --all-features --target wasm32-unknown-unknown`
- optional measured work: `cargo bench -p rustok-ui-i18n --bench lookup`

Next.js Fluent parity surface:
- `cd packages/next-fluent && npm run verify`

Future test work:
- fuzz targets once project fuzz infrastructure exists;
- stress matrices for large malformed/valid catalogs;
- retained benchmark evidence for any further hot-path optimization;
- extension-policy contract tests if the locale model changes.

## Change rules

1. Keep Leptos, Dioxus, Axum, GraphQL, cookie, header, query and routing dependencies out of this crate.
2. Keep locale selection with the host/runtime effective-locale contract.
3. Domain modules own their `.ftl` message files; this crate owns the engine and shared formatting boundary.
4. Do not silently weaken Unicode bidi safety for prettier serialized strings.
5. Do not claim full BCP-47 extension semantics until the locale representation actually preserves them.
6. Keep request-controlled parsing/diagnostics bounded before allocation/logging.
7. Make performance changes from benchmark evidence rather than replacing simple structures speculatively.
8. Treat public enum/type changes as semver work even while the crate remains pre-1.0.
