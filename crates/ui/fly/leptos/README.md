# Fly Leptos

`fly-leptos` is the first browser/framework adapter for Fly. It provides a clean
Leptos-only dependency boundary, reusable full/inline/preview/read-only shells, browser
coordinate value types, hit-test normalization, and lifecycle cleanup registration — plus
`FlyEditorWorkspace` (with `FlyPalette`, `FlyLayerTree`, `FlyCanvasPreview`): a working
editor surface of palette, layer tree, canvas preview, design-token editor, and undo/redo
bound to a `FlyEditor` and the shared `FlyUiStateMachine`, on the shared `fly-ui`
view-models, with host-resolved `EditorMessages` panel labels. With `on_intent` the workspace instead emits typed `BrowserIntentEnvelope`s
(insert/select/undo/redo) for the host's SSR intent endpoint.

It intentionally does not yet claim the Phase 3 gate. Iframe orchestration, real-DOM overlays,
auto-scroll, pointer capture, resize handles, full drag-and-drop, and browser interaction suites
remain open in the central implementation plan.
