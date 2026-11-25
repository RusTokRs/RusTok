# Fly Web

For full public interface and web runtime definitions, see [CRATE_API.md](CRATE_API.md).

`fly-web` is the framework-neutral browser runtime, geometry, hit-testing, and iframe bridge for Fly visual editors.

## Responsibilities

- Define screen, viewport, and canvas coordinates (`BrowserPoint`, `BrowserRect`, `CoordinateTransform`).
- Provide hit-testing, drop candidate normalization, and auto-scroll policies (`hit_test_drop_targets`, `auto_scroll_delta`). Invalid browser bounds and invalid transformed canvas rectangles are excluded before candidates reach the state machine. Auto-scroll is idle for malformed geometry and splits narrow viewports into non-overlapping edge zones.
- Define the typed iframe bridge protocol (`IframeBridgeEnvelope`, `IframeBridgeMessage`) with monotonic sequence and instance validation.
- Own the two origin rules: inbound acceptance compares `event.origin` against a canonical expected origin (`normalize_expected_origin`, which accepts HTTP(S) origins and `OPAQUE_ORIGIN`, and rejects raw non-origin syntax before URL canonicalization, including paths that normalize away, malformed authority delimiters, credentials, queries and fragments), while outbound delivery translates an opaque destination to `*` (`outbound_target_origin`), because the platform rejects `"null"` as a `postMessage` target.
- Implement RAII browser event listener handles (`EventListenerHandle`, `CleanupRegistry`).
- Implement keyboard shortcut translation and resize sessions (`BrowserResizeSession`).
- Implement authenticated real-DOM inline edit grants and request validation, including the
  client-side half of the commit contract: only a changed element emits a request, and the
  value is normalized to the plain text the grant accepts before it is bound.
- Provide the page-scoped `BrowserPeerWriteChannel` used by the classic JS adapter and Rust
  Leptos/Dioxus hosts. Its sender identity combines a document nonce with an editor-root scope,
  so channels for one editor advance together while other tabs or editor roots are treated as
  peers. `new` creates a unique editor-root scope; use `new_scoped` when the Rust host and JS
  adapter represent the same mounted root. Messages contain revision/hash metadata only, never
  document content. The JS adapter also fences explicit server revision-conflict failures when a
  peer notification was missed, and clears the parked fence only after a reload exposes a
  different server revision.

## Known Limitations / Pending Implementation

- **The generic window subscription is not the Page Builder canvas protocol.**
  `FlyCanvasIframe` uses `IframeMessagePort` to request a snapshot from a sandboxed
  `srcdoc` frame, while `IframeJsonSubscription` validates the canvas-specific envelope,
  source window, origin, instance ID and monotonic sequence. `WindowMessageSubscription`
  remains a lower-level generic utility rather than the Page Builder's typed message contract.
  `IframeMessagePort::post` translates an opaque destination through `outbound_target_origin`;
  passing `"null"` as a `postMessage` target is rejected by the platform.
- **Peer-write notifications are advisory, not collaboration or idempotency.** They fence a
  stale editor after another tab confirms a write, but do not merge operations, recover an
  unknown network outcome, or replace the server's revision compare-and-swap and save receipt.
  If BroadcastChannel is unavailable, server revision checks remain the final protection.
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
