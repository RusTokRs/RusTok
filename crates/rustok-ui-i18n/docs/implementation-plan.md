# `rustok-ui-i18n` Implementation Plan

## Focus

Establish `rustok-ui-i18n` as the canonical framework-agnostic UI i18n catalog layer for
Leptos and future Dioxus adapters.

## Current State

- The crate owns catalog building and message resolution.
- `rustok-ui-i18n-leptos` owns shared Leptos adapter boilerplate for module UI packages.
- Locale selection remains host/runtime-owned.

## Improvements

- Keep module UI packages on `rustok-ui-i18n-leptos` instead of package-local catalog boilerplate.
- Add a verifier that prevents new `leptos_i18n` usage in module-owned UI packages.
- Add `rustok-ui-i18n-dioxus` when Dioxus becomes an actual workspace dependency.
- Add catalog parity helpers only if existing i18n verification scripts need a Rust-side API.

## Non-Goals

- No Leptos, Dioxus, Next.js, routing, cookie, header, query or GraphQL dependencies.
- No module-specific copy or business translation keys.
- No ICU/pluralization support until a concrete module requires it.
