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

## Open results

1. **Eliminate custom `unsafe` wakers in favor of `std::task::Waker::noop()`.**
   Done when all custom `RawWaker` / `RawWakerVTable` implementations in tests
   are completely replaced by `std::task::Waker::noop()` (standard in Rust 1.85+)
   or `futures_util::task::noop_waker_ref()`, achieving zero `unsafe` code.
   **Depends on:** Rust 1.85+ compiler (workspace targets Rust 1.96).
   **Verification:** `cargo test -p rustok-ui-transport --lib` and absence of
   `unsafe` blocks in `crates/ui/rustok-ui-transport`.

2. **Standardize test runtime execution.**
   Done when busy-loop spinning `block_on` is removed in favor of standard
   async test macros (`#[tokio::test]`) or `futures::executor::block_on`.
   **Depends on:** Result 1.
   **Verification:** all async unit tests pass cleanly without manual thread yielding.

3. **Establish resilient fallback execution strategy.**
   Done when `execute_with_fallback` allows UI surfaces to try primary transport
   (e.g. native server function) and automatically fall back to secondary
   (GraphQL) with structured diagnostic evidence if the primary path is degraded.
   **Depends on:** Result 1.
   **Verification:** unit tests simulating primary transport failure and verifying
   fallback execution and error reporting.

4. **Standardize transport error taxonomy across modules.**
   Done when module transport facades adopt canonical `UiTransportError`
   conversions instead of defining conflicting `TransportError`/`ApiError`
   aliases.
   **Depends on:** Result 3.
   **Verification:** compilation check across module transport packages.

## Verification

- `cargo test -p rustok-ui-transport --lib`
- `cargo check -p rustok-pricing-storefront`
- Audit confirming zero `unsafe` keyword occurrences in this crate.

## Change rules

1. This crate must remain 100% free of `unsafe` code.
2. Keep all Leptos and Dioxus dependencies out of this crate.
3. Transport path decisions must respect the host-provided `UiTransportPath` contract.
