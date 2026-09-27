# `rustok-ui-i18n` Documentation

`rustok-ui-i18n` is the framework-agnostic UI message catalog boundary for RusToK.

## Purpose

- provide shared Project Fluent catalog construction and key resolution;
- support Leptos and future UI adapters through the same non-reactive API;
- keep locale selection in the host/runtime layer and message resolution in a neutral crate;
- preserve Fluent's Unicode bidi isolation around interpolated values by default.

## Responsibility Zone

- compile embedded `.ftl` resources into concurrent Fluent bundles;
- parse complete Unicode locales, canonicalize CLDR aliases, and project them to Fluent catalog identities;
- expose CLDR directionality and likely-script-aware fallback without host-maintained language lists;
- resolve requested, default, platform and literal fallback paths;
- resolve compound-message values/attributes and report the catalog locale that supplied them;
- validate value/attribute schemas and transitive Fluent references before strict startup;
- expose lenient UI rendering and strict validation/resolution APIs with typed diagnostics;
- provide module declaration and argument-construction macros without framework dependencies.

Runtime requests use ICU4X Unicode locale parsing and CLDR canonicalization. The complete identity,
including extensions, is available through `normalize_unicode_locale`; Fluent catalog selection uses
its language/script/region/variant projection through `normalize_locale_tag`. Catalog declarations
remain extension-free by design. Locale negotiation (headers, cookies, user preferences, q-values)
remains host policy rather than a hidden responsibility of this crate.

## Integration

- Module-owned UI packages use `UiMessages` directly from local `i18n.rs` files without framework adapter crates.
- `rustok-api` does not own or re-export UI i18n helpers.
- Host applications pass the effective locale; this crate does not inspect cookies, headers,
  route query parameters or framework context.
- `@rustok/next-fluent` uses the same bidi-safe interpolation default so Rust and Next.js render
  mixed-direction arguments consistently.

## Verification

- `cargo test -p rustok-ui-i18n`
- `cargo clippy -p rustok-ui-i18n -- -D warnings`
- `cargo check -p rustok-ui-i18n --target wasm32-unknown-unknown`

## Related Documents

- [Root README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Engineering Audit (2026-09-27)](./engineering-audit-2026-09-27.md)
- [Module UI Package Implementation Guide](../../../../docs/UI/module-package-implementation.md)
- [Platform Documentation Map](../../../../docs/index.md)
