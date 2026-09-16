# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` is the framework-agnostic Project Fluent foundation for module-owned UI copy. The
crate owns catalog construction, bounded locale normalization, structured requested/default/platform
fallback, strict and lenient resolution, prepared startup/runtime facades, module macros, typed
initialization diagnostics, concurrent bundle storage and benchmark/test infrastructure.

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

5. **Bounded raw locale-input contract.**
   Runtime lookup, catalog construction and default-locale validation enforce the same 64-byte raw
   input bound before trimming or normalization work. Oversized Rust diagnostics retain only length
   metadata and never copy/log the full payload. `@rustok/next-fluent` applies the corresponding raw
   64-code-unit gate and also bounds oversized configuration error text.

6. **Prepared fail-closed runtime.**
   `UiMessages::prepare` validates once and returns `PreparedUiMessages` backed by the exact strict
   catalog that passed construction. Strict startup requires the normalized default locale to have an
   exact catalog entry.

7. **Inspectable lazy initialization.**
   Lazy `UiMessages` initialization caches one `FluentCatalogBuildReport` in `OnceLock` and preserves
   the existing `fluent_catalog() -> &FluentCatalog` API. `initialization_diagnostics()` exposes the
   same cached report without rebuilding and includes both skipped catalog entries and invalid,
   oversized or missing default-locale configuration.

8. **Prepared per-locale resolution.**
   `UiLocaleTranslator` precomputes one locale fallback chain per request/render scope and reuses it
   across repeated message lookups, avoiding per-key locale parsing/candidate allocation.

9. **Structured locale fallback.**
   Rust fallback operates on `LanguageIdentifier` structure rather than serialized suffix chopping:
   exact locale -> all variants removed as one layer -> region removed -> script removed -> language.
   This prevents canonical variant sorting from manufacturing arbitrary partial-variant parents.
   `@rustok/next-fluent` now mirrors that shape with `Intl.Locale`, while additionally allowing an
   exact extension-bearing match and then probing the extension-free `baseName`.

10. **Strict and lenient message resolution.**
    `try_resolve_fluent_message` / strict format APIs report missing keys and Fluent formatting errors;
    lenient rendering returns the caller's explicit literal fallback and never exposes partial malformed output.

11. **Catalog/key collision contract.**
    Dotted application keys and kebab Fluent IDs are explicitly one logical identity (`a.b` == `a-b`).
    Duplicate normalized locales are strict errors and lenient first-wins. Duplicate message IDs are
    rejected by Fluent resource validation.

12. **Safe dotted-key conversion.**
    `with_kebab_key` keeps the 128-byte stack path without `unsafe`; the stack slice is validated with
    safe UTF-8 conversion and an unexpected failure falls back to the allocating replacement path.

13. **Macro/public API contract coverage.**
    Exported macros have external-consumer rustdoc compile-pass/compile-fail coverage, including renamed
    crate `$crate` hygiene. Prepared runtime types are exposed from the crate root.

14. **Property and concurrency coverage.**
    Property tests cover locale normalization/candidate invariants and optimized dotted-key conversion.
    Native concurrency tests race first `OnceLock` initialization and steady-state Fluent plural lookup.

15. **Performance measurement infrastructure.**
    Criterion covers locale-candidate/prepared construction, direct/default/missing/interpolated lookup,
    22-locale catalog shape and 1,000-message catalog shape. Catalog construction parses each locale once.

16. **Next locale-boundary hardening.**
    `@rustok/next-fluent` performs `Accept-Language` negotiation in a single pass instead of allocating
    and sorting a request-sized candidate array. HTTP qvalues are validated against the RFC 9110
    `0..1` / maximum-three-fractional-digit grammar so malformed weights cannot gain permissive priority.

17. **Focused verification.**
    Path-filtered Rust and Next Fluent workflows provide package-level signals independent from unrelated
    monorepo baseline failures. Rust verification includes format, tests/doctests, Clippy and WASM check.

18. **WASM-friendly production boundary.**
    Production code is in-memory and has no runtime filesystem dependency; `wasm32-unknown-unknown`
    remains a supported compile target.

19. **Forward-compatible public error enums.**
    `BundleBuildError` and `I18nError` are non-exhaustive before 1.0. Downstream consumers can still
    inspect stable variants but must keep a wildcard arm, allowing new typed diagnostics to be added
    without turning every exhaustive match into a future semver blocker.

20. **Deterministic stress validation.**
    Integration stress tests cover a 1,000-message Fluent resource, a 64-locale strict catalog, and a
    mixed 128-entry lenient batch containing valid, invalid-locale, oversized-locale and malformed-FTL
    inputs. The batch contract verifies usable-entry preservation, typed diagnostic ordering, and
    payload-free `LocaleTooLong` metadata at scale.

## Remaining engineering work

### 1. Locale model and extension semantics

Rust catalog identity remains `unic_langid::LanguageIdentifier`. It models language, optional script,
region and variants, but not Unicode/private-use extensions. Next canonicalization can preserve those
extensions through `Intl`, but that does not give Rust catalogs extension semantics.

Remaining decisions:
- decide whether extension-aware Rust catalog identity is ever required;
- otherwise keep extension selection explicitly host-owned and document Rust as intentionally
  `LanguageIdentifier`-only;
- if the Rust representation is widened later, define exact cross-runtime extension identity/fallback
  rules rather than layering ad-hoc extension stripping onto catalog lookup.

### 2. Public API / semver surface before 1.0

The crate is currently workspace version `0.1.0` and publicly exposes modules, Fluent types,
`unic_langid::LanguageIdentifier`, `FluentCatalog` internals and helper functions in addition to the
intended high-level message/runtime facades. The public error enums are now explicitly non-exhaustive.
The workspace dependency graph demonstrates broad consumption of this crate, so existing exports remain
compatibility surface until symbol-level consumers can be migrated deliberately.

Remaining work before a stable release:
- complete symbol-level inventory of public modules, dependency types and low-level helpers;
- decide which dependency types are intentional public contract versus implementation leakage;
- prefer additive facade APIs and reserve removals/type wrapping for an explicit migration window;
- only narrow existing public exports after concrete consumer migration evidence exists.

### 3. Fuzz validation

Property, malformed-input, concurrency and deterministic stress matrices now cover the known contracts,
but generated cases are still needed to explore parser/state combinations that hand-written fixtures do
not enumerate.

Next steps:
- add fuzz targets for locale normalization/candidate construction and FTL/catalog ingestion if repository
  fuzz infrastructure is approved;
- keep fuzz corpora bounded so failures remain reproducible and diagnostics cannot amplify generated input;
- preserve any minimized regression input as a deterministic integration test after a fuzz finding.

### 4. Retained benchmark evidence

The benchmark matrix exists, but optimization decisions should retain actual before/after numbers when a
hot-path change is proposed. Do not replace `BTreeMap`, Fluent storage, or candidate representation based
on intuition alone.

### 5. Locale-aware domain formatting

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
- retained benchmark evidence for any further hot-path optimization;
- extension-policy contract tests if the Rust locale model changes.

## Change rules

1. Keep Leptos, Dioxus, Axum, GraphQL, cookie, header, query and routing dependencies out of this crate.
2. Keep locale selection with the host/runtime effective-locale contract.
3. Domain modules own their `.ftl` message files; this crate owns the engine and shared formatting boundary.
4. Do not silently weaken Unicode bidi safety for prettier serialized strings.
5. Do not claim full Rust BCP-47 extension semantics until the locale representation actually preserves them.
6. Keep request-controlled parsing/diagnostics bounded before allocation/logging where the API owns that input boundary.
7. Make performance changes from benchmark evidence rather than replacing simple structures speculatively.
8. Treat public enum/type changes as semver work even while the crate remains pre-1.0.
