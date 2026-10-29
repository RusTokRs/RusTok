# fly-web / CRATE_API

## Public surface

### Geometry & Hit-Testing
- `BrowserPoint`, `BrowserRect`: screen and canvas geometric primitive models.
- `CoordinateTransform`: matrix/offset mapper translating client pointer events to canvas space.
- `BrowserHitTarget`, `BrowserDropTarget`: drop candidate descriptors with positional weighting.
- `hit_test_drop_targets(...)`, `resolve_drop_position(...)`: deterministic drop target resolution.
- `AutoScrollDelta`, `AutoScrollPolicy`, `auto_scroll_delta(...)`: viewport auto-scrolling calculations.

### Iframe Protocol & Origin Security
- `FLY_IFRAME_PROTOCOL`: protocol version identifier.
- `IframeBridgeEnvelope`, `IframeBridgeMessage`: postMessage protocol envelope with sequence verification.
- `OPAQUE_ORIGIN`: opaque iframe origin constant (`"null"`).
- `normalize_expected_origin(...)`, `outbound_target_origin(...)`: origin sanitizers preventing cross-origin data leakage.

### Real-DOM Inline Editing & Lifecycle
- `AuthenticatedInlineEditGrant`, `AuthenticatedInlineEditRequest`: cryptographically or capability-scoped inline edit requests.
- `CleanupRegistry`: RAII resource manager for unregistering DOM listeners and observers.
- `BrowserResizeSession`: pointer drag session manager for interactive element resizing.

## Invariants

- **Safe Code Boundary**: The crate denies unsafe code (`#![deny(unsafe_code)]`).
- **Origin Boundary Enforcement**: Inbound postMessage events are validated against expected origins; outbound messages translate opaque origins (`"null"`) to `*` to satisfy browser security specifications.
- **Framework Neutrality**: Pure browser runtime and math primitives without dependencies on Leptos, Dioxus, or React.
