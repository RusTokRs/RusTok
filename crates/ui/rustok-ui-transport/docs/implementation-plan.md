# Implementation Plan for `rustok-ui-transport`

## Current state

`rustok-ui-transport` owns framework-agnostic UI transport path selection,
transport error models, and execution envelopes (`UiTransportPath`,
`UiTransportResult`, `execute_selected_transport`). Module transport facades
use this crate to select between native server functions and GraphQL. Currently,
its test suite and downstream modules (such as `rustok-pricing`) contain
hand-rolled `unsafe` wakers (`RawWaker`/`RawWakerVTable`) and busy-loop `block_on`
implementations with `thread::yield_now()`.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This crate defines the execution contract between UI adapters and underlying
  transports. It owns neither domain data schemas nor UI components.

## Completed foundations

1. **Zero-unsafe codebase and standard test runtime.**
   All custom `RawWaker` / `RawWakerVTable` implementations and busy-loop spinning
   `block_on` are completely eliminated in favor of standard async test macros
   (`#[tokio::test]`), achieving 100% safe, clean test execution.

2. **Resilient fallback execution strategy.**
   `execute_with_fallback` and `execute_transport_policy` allow UI surfaces to try
   primary transport (native server functions) and automatically fall back to
   secondary (GraphQL) with structured diagnostic evidence (`UiTransportError::fallback_failed`)
   when primary is degraded, failing closed if both transports fail.

## Open results

1. **Standardize transport error taxonomy across modules.**
   Done when module transport facades adopt canonical `UiTransportError`
   conversions instead of defining conflicting `TransportError`/`ApiError`
   aliases.
   **Verification:** compilation check across module transport packages.

## Verification

- `cargo test -p rustok-ui-transport --lib`
- `cargo check -p rustok-pricing-storefront`
- Audit confirming zero `unsafe` keyword occurrences in this crate.

## Change rules

1. This crate must remain 100% free of `unsafe` code.
2. Keep all Leptos and Dioxus dependencies out of this crate.
3. Transport path decisions must respect the host-provided `UiTransportPath` contract.
