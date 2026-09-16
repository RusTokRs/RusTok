# `rustok-ui-i18n` Documentation

`rustok-ui-i18n` is the framework-agnostic UI message catalog boundary for RusToK.

## Purpose

- provide shared Project Fluent (`.ftl`) catalog construction and key resolution;
- support Leptos and future Dioxus UI adapters through the same non-reactive API;
- keep locale selection in the host/runtime layer and message resolution in a neutral crate.

## Responsibility Zone

- Project Fluent catalog parsing and validation;
- locale tag normalization for catalog lookup;
- effective-locale, default-locale and platform fallback resolution;
- strict validation/resolution APIs for tests and CI;
- literal fallback text for lenient UI rendering when a key or formatting result is unavailable.

## Integration

- Module-owned UI packages use `UiMessages` directly from local `i18n.rs` files via `declare_module_i18n!`.
- `UiMessages::validate` and `bundle::try_build_fluent_catalog` provide fail-closed validation for tests and CI.
- `UiMessages::try_format` exposes formatting failures instead of silently rendering malformed output.
- `rustok-api` does not own or re-export UI i18n helpers.
- Host applications pass the effective locale; this crate does not inspect cookies, headers,
  route query parameters or framework context.

## Verification

- `cargo test -p rustok-ui-i18n`

## Related Documents

- [Root README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Module UI Package Implementation Guide](../../../docs/UI/module-package-implementation.md)
- [Platform Documentation Map](../../../docs/index.md)
