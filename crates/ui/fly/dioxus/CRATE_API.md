# fly-dioxus / CRATE_API

## Public surface

### Shell & Editor Components
- `FlyFullEditor`: top-level full-screen visual editor shell (`role="application"`).
- `FlyInlineEditor`: container shell for contextual inline page-editing sessions.
- `FlyPreview`: responsive live canvas preview container.
- `FlyReadOnly`: non-editable presentation container.
- `FlyEditorWorkspace`: composite workspace component binding palette, layer tree, canvas preview, design-token editor, and undo/redo history.

### Re-exported Web Runtime & Geometry
- Re-exports core types from `fly-web` (`BrowserPoint`, `BrowserRect`, `CoordinateTransform`, `BrowserDropTarget`, `IframeBridgeEnvelope`, `CleanupRegistry`, etc.).

## Invariants

- **Safe Code Boundary**: The crate denies unsafe code (`#![deny(unsafe_code)]`).
- **Framework Separation**: Strictly encapsulates Dioxus 0.7 rendering; does not depend on Leptos or server persistence.
- **Accessibility Invariants**: Editor shells emit standard accessible roles (`role="application"`) and ARIA labels.
