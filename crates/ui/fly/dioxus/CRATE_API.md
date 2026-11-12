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
- `FlyPalette`: Category-grouped block palette honoring `CapabilityState` (`edit` for insert, `drag_drop` for drag) and contribution-access filtering. Renders the same item, insert-control and drag-control markup as `fly-leptos`; the drag control appears only when the host passes `on_begin_drag`.
- `FlyLayerTree`: Interactive layer hierarchy tree with selection highlighting and navigation. Selectable rows carry `data-fly-component-id` and `data-fly-action="select-component"`, as in `fly-leptos`.
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
