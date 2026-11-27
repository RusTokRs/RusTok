# Crate: `fly-dioxus`

SSR and Web/Desktop Dioxus 0.7 adapter foundation and component shells for Fly visual editors.

## 1. Public API Surface

### Shell & Editor Components
- `FlyFullEditor`: Top-level full-screen visual editor shell (`role="application"`).
- `FlyInlineEditor`: Container shell for contextual inline page-editing sessions.
- `FlyPreview`: Responsive live canvas preview container.
- `FlyReadOnly`: Non-editable presentation container.
- `FlyEditorWorkspace`: Composite workspace component binding palette, layer tree, canvas preview, design-token editor, and undo/redo history with typed `on_error` and `on_intent` support.
- `FlyContributionSummary`: Summary badge rendering loaded block, dynamic slot, and extension counts with accessible status semantics.
- `FlyPalette`: Category-grouped block palette honoring `CapabilityState` (`edit` for insert, `drag_drop` for drag) and contribution-access filtering. Drag controls use native HTML5 `dragstart`/`dragend` and appear as draggable only when both `on_begin_drag` and `on_cancel_drag` are provided.
- `FlyLayerTree`: Interactive layer hierarchy tree with selection highlighting and navigation. Selectable rows carry `data-fly-component-id` and `data-fly-action="select-component"`; optional move sources use native HTML5 drag events and require begin/cancel callbacks.
- Hydration boundary: palette and layer-tree roots render data-fly-wasm-hydrated=false in SSR and switch to true after Dioxus hydration. Only Dioxus-owned controls are marked; raw FlyCanvasPreview remains outside those islands so classic JS can continue handling its component selection and drag interactions.
- `FlyThemeTokens`: Read-only design token listing displaying token keys and resolved values.
- `FlyThemeTokenEditor`: Interactive design token management panel supporting upsert, remove, and error dispatch.
- `FlyCanvasPreview`: Responsive live canvas preview container rendering the active page AST into styled HTML with error fallback.

### Re-exported Web Runtime & Geometry
- Re-exports core types from `fly-web`: `BrowserPoint`, `BrowserRect`, `CoordinateTransform`, `BrowserDropTarget`, `BrowserHitTarget`, `IframeBridgeEnvelope`, `CleanupRegistry`, `PointerSample`, `DropZonePolicy`, `AutoScrollPolicy`, etc.

## 2. Invariants & Business Rules
- **Safe Code Boundary**: The crate denies unsafe code (`#![deny(unsafe_code)]`).
- **Framework Encapsulation**: Strictly encapsulates Dioxus 0.7 rendering; does not depend on Leptos or server persistence.
- **Accessibility Invariants**: Editor shells emit standard accessible roles (`role="application"`) and ARIA labels.
- **SSR-Friendly Defaults**: Crate compiles without forced renderer dependencies by default; enables targeted feature activation (`web` or `desktop`).

## 3. Error Handling & Edge Cases
- Fail-closed geometry handling: invalid coordinates or non-invertible matrices fallback safely without panicking.
- Inline edit request authentication errors map to typed `InlineEditContractError`.
- Clean DOM unmount and listener cleanup via `CleanupRegistry`.

## 4. Security & Isolation Invariants
- Sandboxed iframe bridge communication enforces `OPAQUE_ORIGIN` checks and normalized origins (`normalize_expected_origin`).
- Authenticated inline editing requires signed grants (`AuthenticatedInlineEditGrant`).
- Bounded text payload limits (`MAX_INLINE_TEXT_BYTES`).

## 5. Contract Tests & Verification
- Unit test suite verifying accessibility and re-exported web runtime type contracts.
- `src/contract_tests.rs`: Direct verification of package metadata, re-exported geometric contracts, origin normalization, and accessibility roles.

## Isolated canvas

- `FlyCanvasIframe`: sandboxed `srcdoc` iframe that emits raw bridge messages only after exact
  source-window, expected-origin, protocol, instance and monotonic-sequence validation. The
  subscription is recreated after iframe reload; the host interprets messages and owns geometry,
  hit-testing, drop legality and mutations.
- `FlyEditorWorkspace.canvas`: optional host-owned canvas slot. When absent, the workspace keeps
  rendering `FlyCanvasPreview`.
