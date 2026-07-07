# `rustok-ui-i18n` Implementation Plan

## Focus

Establish `rustok-ui-i18n` as the canonical framework-agnostic UI i18n catalog layer for
Leptos and future Dioxus adapters.

## Current State

- The crate owns catalog building and message resolution.
- `rustok-api` re-exports the public helpers for compatibility with existing module UI packages.
- Locale selection remains host/runtime-owned.

## Improvements

- Migrate module UI packages from `rustok_api::{build_ui_message_catalog, ...}` imports to
  direct `rustok_ui_i18n::{...}` imports in small batches.
- Add a verifier that prevents new `leptos_i18n` usage in module-owned UI packages.
- Add catalog parity helpers only if existing i18n verification scripts need a Rust-side API.

## Non-Goals

- No Leptos, Dioxus, Next.js, routing, cookie, header, query or GraphQL dependencies.
- No module-specific copy or business translation keys.
- No ICU/pluralization support until a concrete module requires it.

