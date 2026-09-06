# Implementation Plan for `rustok-ui-i18n`

## Current state

`rustok-ui-i18n` owns framework-agnostic JSON catalog construction, locale-tag
normalization, message lookup, and fallback resolution. It consumes a
host-provided effective locale; it does not select locale from framework state,
cookies, headers, routes, or transport. `rustok-ui-i18n-leptos` owns the shared
Leptos adapter layer.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This crate owns neither framework hooks nor module business copy, routing,
  transport, or locale selection policy.

## Open results

1. **Lock shared Leptos catalog adoption.** Done when a focused guard prevents
   new package-local catalog boilerplate or direct `leptos_i18n` use in
   module-owned UI packages where `rustok-ui-i18n-leptos` is applicable.
   **Depends on:** an approved repository-wide UI ownership rule.
   **Verification:** the new focused source verifier plus
   `cargo test -p rustok-ui-i18n-leptos --lib`.
2. **Add a Dioxus adapter only for a real workspace host.** Done when an
   approved Dioxus consumer needs reactive catalog/context integration and the
   adapter leaves this crate framework-agnostic.
   **Depends on:** Dioxus entering the workspace with a concrete owner.
   **Verification:** targeted adapter integration tests and dependency audit.
3. **Add Rust-side catalog parity helpers only for a demonstrated verifier
   gap.** Done when an existing i18n verification path cannot use current
   catalog APIs and the added helper removes duplicated logic without becoming
   a module-copy registry.
   **Depends on:** a concrete failing or duplicated verification path.
   **Verification:** focused catalog fixture tests.

## Verification

- `cargo test -p rustok-ui-i18n --lib`
- `cargo test -p rustok-ui-i18n-leptos --lib`

## Change rules

1. Keep framework dependencies in sibling adapter crates.
2. Keep locale selection with the host/runtime effective-locale contract.
3. Do not add module translation keys, business copy, or transport policy.
