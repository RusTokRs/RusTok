# fly-leptos / CRATE_API

## Public surface

### Shell & Editor Components
- `FlyFullEditor`: top-level full-screen visual editor shell (`role="application"`).
- `FlyInlineEditor`: container shell for contextual inline page-editing sessions.
- `FlyPreview`: responsive live canvas preview container.
- `FlyReadOnly`: non-editable presentation container.
- `FlyEditorWorkspace`: composite workspace component binding palette, layer tree, canvas preview, design-token editor, and undo/redo history.
- `FlyPalette`, `FlyLayerTree`, `FlyCanvasPreview`, `FlyPageTabs`, `FlyStyleRules`, `FlyPageProperties`, `FlyComponentProperties`, `FlyAssets`, `FlyThemeTokens`, `FlyThemeTokenEditor`, `FlyContributionSummary`: modular subcomponents composing the editor workspace.

### Re-exported Web Runtime & Geometry
- Re-exports core types from `fly-web` (`BrowserPoint`, `BrowserRect`, `CoordinateTransform`, `BrowserDropTarget`, `IframeBridgeEnvelope`, `CleanupRegistry`, etc.).
- Optional WASM browser runtime hooks (`BrowserResizeSession`, `key_stroke_from_event`, `EventListenerHandle`, `IframeMessagePort`, etc.) available under `feature = "wasm-client"`.

## Invariants

- **Safe Code Boundary**: The crate denies unsafe code (`#![deny(unsafe_code)]`).
- **SSR-First Parity**: Layout shells and workspace components render static HTML and ARIA landmarks identically in SSR and client hydration.
- **WASM Client Feature Gating**: Browser-specific DOM manipulation and listener subscriptions require explicit `wasm-client` feature opt-in.
