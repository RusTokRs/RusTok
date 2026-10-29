# fly-leptos / CRATE_API

## Public surface

### Components & Workspaces
- `FlyEditorWorkspace`: interactive Leptos 0.8 visual editor workspace binding palette, layer tree, design token manager, and preview canvas.
- `FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`: presentational shell components.
- `FlyCanvasPreview`: reactive canvas rendering document models with interactive selection highlights.

### Re-exports
- Re-exports core editor, UI state, and web contracts from `fly`, `fly-ui`, and `fly-web`.

## Invariants

- **Safe Rust**: The entire crate forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Framework Separation**: Dedicated Leptos 0.8 adapter with zero Dioxus imports and zero database persistence dependencies.
- **SSR Parity**: Full SSR rendering support with tachys reactivity and typed browser intent dispatch.
