# Fly Web

`fly-web` is the framework-neutral browser runtime, geometry, hit-testing, and iframe bridge for Fly visual editors.

## Responsibilities

- Define screen, viewport, and canvas coordinates (`BrowserPoint`, `BrowserRect`, `CoordinateTransform`).
- Provide hit-testing, drop candidate normalization, and auto-scroll policies (`hit_test_drop_targets`, `auto_scroll_delta`).
- Define the typed iframe bridge protocol (`IframeBridgeEnvelope`, `IframeBridgeMessage`) with monotonic sequence and instance validation.
- Implement RAII browser event listener handles (`EventListenerHandle`, `CleanupRegistry`).
- Implement keyboard shortcut translation and resize sessions (`BrowserResizeSession`).
- Implement authenticated real-DOM inline edit grants and request validation.

## Architectural Boundaries

This crate is strictly framework-neutral:
- **Zero dependencies** on Leptos, Dioxus, or any specific UI rendering library.
- Compatible with WASM browser environments (`web-sys`, `wasm-bindgen`) and headless SSR runtimes.
- Consumed by both `fly-leptos` and `fly-dioxus` adapters.
