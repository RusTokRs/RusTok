# Crate: `fly-web`

Framework-neutral browser runtime, geometric transforms, hit-testing, and iframe postMessage bridge contracts for Fly visual editors.

## 1. Public API Surface

### Geometry & Hit-Testing
- `BrowserPoint`, `BrowserRect`: Screen and canvas geometric primitive models with finite validation (`is_finite`).
- `CoordinateTransform`: Matrix/offset mapper translating client pointer events to canvas space with zoom normalization.
- `BrowserHitTarget`, `BrowserDropTarget`: Drop candidate descriptors with positional weighting and axis alignment.
- `hit_test_drop_targets(...)`, `resolve_drop_position(...)`: Deterministic drop target resolution.
- `AutoScrollDelta`, `AutoScrollPolicy`, `auto_scroll_delta(...)`: Viewport auto-scrolling calculations.

### Iframe Protocol & Origin Security
- `FLY_IFRAME_PROTOCOL`: Protocol version identifier (`"fly_iframe"`).
- `IframeBridgeEnvelope`, `IframeBridgeMessage`: postMessage protocol envelope with sequence verification and replay defense.
- `OPAQUE_ORIGIN`: Opaque iframe origin constant (`"null"`).
- `normalize_expected_origin(...)`, `outbound_target_origin(...)`: Origin sanitizers preventing cross-origin data leakage.

### Real-DOM Inline Editing & Lifecycle
- `AuthenticatedInlineEditGrant`, `AuthenticatedInlineEditRequest`: Capability-scoped inline edit requests with token verification.
- `CleanupRegistry`: RAII resource manager for unregistering DOM listeners and observers.
- `BrowserResizeSession`: Pointer drag session manager for interactive element resizing.

## 2. Invariants & Business Rules
- **Safe Code Boundary**: The crate denies unsafe code (`#![deny(unsafe_code)]`).
- **Origin Boundary Enforcement**: Inbound postMessage events are validated against expected origins; outbound messages translate opaque origins (`"null"`) to `*` to satisfy browser security specifications.
- **Framework Neutrality**: Pure browser runtime and math primitives without dependencies on Leptos, Dioxus, or React.
- **Fail-Closed Geometry**: Non-finite coordinates (NaN, Inf) or degenerate transforms hit nothing and produce no drop positions.

## 3. Error Handling & Edge Cases
- Degenerate rects, broken zoom, and out-of-range pointer coordinates gracefully clamp without panicking.
- Inline edit request validation rejects NUL bytes, bidirectional overrides, and control characters.
- Replay and sequence verification rejects out-of-order or duplicate iframe bridge envelopes.

## 4. Security & Isolation Invariants
- Sandboxed iframe bridge isolates host editor from sandboxed document runtime.
- Secret tokens in `AuthenticatedInlineEditGrant` are redacted from `Debug` output to prevent leakage into logs.
- Bounded text payload limits (`MAX_INLINE_TEXT_BYTES`) protect against memory exhaustion.

## 5. Contract Tests & Verification
- Unit test suite: 30 tests covering candidate ranking, coordinate transform roundtripping, auto-scroll bounds, origin normalization, and inline edit grant security.
- `src/contract_tests.rs`: Direct verification of package metadata, geometric contracts, origin sanitization, and cleanup registry mechanics.
