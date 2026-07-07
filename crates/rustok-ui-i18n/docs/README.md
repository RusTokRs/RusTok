# `rustok-ui-i18n` Documentation

`rustok-ui-i18n` is the framework-agnostic UI message catalog boundary for RusToK.

## Purpose

- provide shared UI message catalog construction and key resolution;
- support Leptos and future Dioxus UI adapters through the same non-reactive API;
- keep locale selection in the host/runtime layer and message resolution in a neutral crate.

## Responsibility Zone

- nested JSON locale bundle flattening;
- locale tag normalization for catalog lookup;
- effective-locale, default-locale and platform fallback resolution;
- literal fallback text when a key is missing.

## Integration

- Leptos module-owned UI packages should use `rustok-ui-i18n-leptos` from local
  `i18n.rs` files.
- Framework adapters depend on this crate directly; `rustok-api` does not own or
  re-export UI i18n helpers.
- Host applications pass the effective locale; this crate does not inspect cookies, headers,
  route query parameters or framework context.

## Verification

- `cargo test -p rustok-ui-i18n --lib`
- `cargo test -p rustok-ui-i18n-leptos --lib`

## Related Documents

- [Root README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Module UI Package Implementation Guide](../../../docs/UI/module-package-implementation.md)
- [Platform Documentation Map](../../../docs/index.md)
