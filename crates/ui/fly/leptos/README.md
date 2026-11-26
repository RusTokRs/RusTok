# Fly Leptos

For full public interface and component definitions, see [CRATE_API.md](CRATE_API.md).

`fly-leptos` is the first browser/framework adapter for Fly. It provides a clean
Leptos-only dependency boundary, reusable full/inline/preview/read-only shells, browser
coordinate value types, hit-test normalization, and lifecycle cleanup registration — plus
`FlyEditorWorkspace` (with `FlyPalette`, `FlyLayerTree`, `FlyCanvasPreview`): a working
editor surface of palette, layer tree, canvas preview, design-token editor, and undo/redo
bound to a `FlyEditor` and the shared `FlyUiStateMachine`, on the shared `fly-ui`
view-models, with host-resolved `EditorMessages` panel labels. With `on_intent` the workspace instead emits typed `BrowserIntentEnvelope`s
(insert/select/undo/redo) for the host's SSR intent endpoint.

`FlyCanvasPreview` renders the project stylesheet as an inline `<style>` element, as does the
workspace around it. Both accept `csp_nonce`; a host that serves the editor under a nonce policy
(`style-src 'self' {nonce}`) must pass its request nonce, otherwise the browser drops the
stylesheet and the authoring canvas renders unstyled while the published page looks right.
`AdapterWorkspacePanel` in `rustok-page-builder-admin` reads it from the `UiCspNonce` context.

`FlyPalette` and `FlyLayerTree` are the producer side of the SSR control contract: on a
server-rendered page their Leptos handlers do not exist, and `fly-browser.js` drives them
through the `data-fly-*` attributes they render. Renaming one of those attributes disables
the control in the default runtime; `scripts/verify/verify-fly-ssr-control-contract.mjs` and
the render tests in `src/editor.rs` fail first. See
[the decision](../../../../DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md).

It intentionally does not yet claim the Phase 3 gate. Iframe orchestration, real-DOM overlays,
auto-scroll, pointer capture, resize handles, full drag-and-drop, and browser interaction suites
remain open in the central implementation plan.

## Controlled Page Builder workspace updates

An intent-mode consumer may supply `controlled_document`, `controlled_selection`,
`controlled_page`, `controlled_can_undo`, `controlled_can_redo`, and `intent_context` as
Leptos `Signal`s. `intent_context` carries a `WorkspaceIntentContext` containing the page ID,
revision and project hash for the controlled projection; the workspace attaches it to each typed
intent, and the host must not fill missing values by re-reading the live controller.
The workspace synchronizes the canonical controller document and selection into its internal
FlyEditor projection, and the active page into its navigation signal. Standalone mode retains
its own mutable FlyEditor/history; no external synchronization occurs without `on_intent`.
Page tabs are derived reactively from the editor's live document instead of the initial prop.
No entire-workspace remount is needed for a normal user edit.

The optional `controlled_capabilities` signal updates command affordances in a controlled
Leptos workspace without remounting. Page tabs, palette, theme tokens, component properties,
scoped styles, assets, Undo and Redo now read the current effective policy instead of a stale
first-mount capability snapshot. Dispatch-time authorization still applies.

In the default SSR/browser-JS mode, Undo and Redo remain available through the
`intent:undo` and `intent:redo` browser controls. The edit/preview mode switch and panels
that require Leptos event handlers stay hidden until WASM hydration.
