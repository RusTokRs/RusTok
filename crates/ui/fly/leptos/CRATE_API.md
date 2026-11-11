# Crate: `fly-leptos`

SSR-first Leptos 0.8 adapter foundation, editor component shells, and browser runtime integration for Fly visual editors.

## 1. Public API Surface

### Shell & Workspace Components
- `FlyFullEditor`: Top-level full-screen visual editor shell (`role="application"`).
- `FlyInlineEditor`: Container shell for contextual inline page-editing sessions.
- `FlyPreview`: Responsive live canvas preview container.
- `FlyReadOnly`: Non-editable presentation container.
- `FlyEditorWorkspace`: Composite workspace component binding palette, layer tree, canvas preview, design-token editor, and undo/redo history.
- Modular panels: `FlyPalette`, `FlyLayerTree`, `FlyCanvasPreview`, `FlyPageTabs`, `FlyStyleRules`, `FlyPageProperties`, `FlyComponentProperties`, `FlyAssets`, `FlyThemeTokens`, `FlyThemeTokenEditor`, `FlyContributionSummary`.

### Re-exported Web Runtime & Geometry
- Re-exports core types from `fly-web`: `BrowserPoint`, `BrowserRect`, `CoordinateTransform`, `BrowserDropTarget`, `IframeBridgeEnvelope`, `CleanupRegistry`, `PointerSample`, `DropZonePolicy`, `AutoScrollPolicy`, etc.
- Optional WASM browser runtime hooks (`BrowserResizeSession`, `key_stroke_from_event`, `EventListenerHandle`, `IframeMessagePort`, etc.) available under `feature = "wasm-client"`.

## 2. Invariants & Business Rules
- **Safe Code Boundary**: The crate denies unsafe code (`#![deny(unsafe_code)]`).
- **SSR-First Parity**: Layout shells and workspace components render static HTML and ARIA landmarks identically in SSR and client hydration.
- **SSR Control Contract (producer side)**: `FlyPalette` renders each block as an item with `data-fly-block-id`, `data-fly-can-insert` and `data-fly-can-drag`, an insert control with `data-fly-action="insert-block"`, and — only when the host passes `on_begin_drag`, which `FlyEditorWorkspace` does in intent mode — a drag control with `data-fly-action="begin-block-drag"`. `FlyLayerTree` renders `data-fly-component-id` and `data-fly-action="select-component"` on selectable rows only. `fly-browser.js` binds exactly these on a server-rendered page; `scripts/verify/verify-fly-ssr-control-contract.mjs` fails when the two sides differ.
- **WASM Client Feature Gating**: Browser-specific DOM manipulation and listener subscriptions require explicit `wasm-client` feature opt-in.
- **Root Entrypoint**: Canonical entrypoint at `src/root.rs` cleanly separating host SSR shells from optional WASM browser interactions.

## 3. Error Handling & Edge Cases
- Fail-closed geometry handling: invalid coordinates or non-finite rects fallback safely without panic.
- Layer tree indent scaling is bounded to prevent unbounded style cascades.
- Inline edit request validation maps to typed `InlineEditContractError`.

## 4. Security & Isolation Invariants
- Sandboxed iframe bridge communication enforces `OPAQUE_ORIGIN` checks and normalized origins (`normalize_expected_origin`).
- Bounded text payload limits (`MAX_INLINE_TEXT_BYTES`).
- Authenticated inline editing requires signed grants (`AuthenticatedInlineEditGrant`).

## 5. Contract Tests & Verification
- Unit test suite verifying bounded layer indent scaling, canvas preview modes, and workspace instantiation.
- `src/contract_tests.rs`: Direct verification of package metadata, re-exported geometric contracts, origin normalization, and accessibility roles.
