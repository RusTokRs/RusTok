# Fly Web

For full public interface and web runtime definitions, see [CRATE_API.md](CRATE_API.md).

`fly-web` is the framework-neutral browser runtime, geometry, hit-testing, and iframe bridge for Fly visual editors.

## Responsibilities

- Define screen, viewport, and canvas coordinates (`BrowserPoint`, `BrowserRect`, `CoordinateTransform`).
- Provide hit-testing, drop candidate normalization, and auto-scroll policies (`hit_test_drop_targets`, `auto_scroll_delta`).
- Define the typed iframe bridge protocol (`IframeBridgeEnvelope`, `IframeBridgeMessage`) with monotonic sequence and instance validation.
- Own the two origin rules: inbound acceptance compares `event.origin` against a normalized expected origin (`normalize_expected_origin`, which accepts `OPAQUE_ORIGIN`), while outbound delivery translates an opaque destination to `*` (`outbound_target_origin`), because the platform rejects `"null"` as a `postMessage` target.
- Implement RAII browser event listener handles (`EventListenerHandle`, `CleanupRegistry`).
- Implement keyboard shortcut translation and resize sessions (`BrowserResizeSession`).
- Implement authenticated real-DOM inline edit grants and request validation, including the
  client-side half of the commit contract: only a changed element emits a request, and the
  value is normalized to the plain text the grant accepts before it is bound.

## Known Limitations / Pending Implementation

- **The outbound half of the iframe bridge has no caller inside this repository.**
  `IframeMessagePort` and `WindowMessageSubscription` are covered by their own tests only; the Page
  Builder canvas speaks its own subscription protocol (`IframeJsonSubscription` in
  `rustok-page-builder/admin/src/editor/isolated_canvas.rs`). `IframeMessagePort::post` translates an
  opaque destination — a `sandbox="allow-scripts"` `srcdoc` frame — through `outbound_target_origin`,
  which is what makes the port deliver at all: it used to pass `"null"`, a `postMessage` target the
  platform rejects.
- **The wasm-only surface is type-checked, not executed, in CI.** `browser_interaction`,
  `browser_runtime`, and `real_dom_inline`'s editor mounting are gated by
  `cfg(target_arch = "wasm32")` plus the `wasm-client` feature, so the `Check Fly browser runtime on
  wasm32` job compiles them and `cargo test` never runs them. Browser behaviour is asserted only by
  the Playwright/browser-evidence specs under `apps/next-admin/tests`, in the Browser E2E workflow.

## Architectural Boundaries

This crate is strictly framework-neutral:
- **Zero dependencies** on Leptos, Dioxus, or any specific UI rendering library.
- Compatible with WASM browser environments (`web-sys`, `wasm-bindgen`) and headless SSR runtimes.
- Consumed by both `fly-leptos` and `fly-dioxus` adapters.
