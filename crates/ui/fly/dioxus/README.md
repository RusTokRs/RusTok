# Fly Dioxus

For full public interface and component definitions, see [CRATE_API.md](CRATE_API.md).

`FlyCanvasPreview` and `FlyEditorWorkspace` render the project stylesheet as an inline `style`
element and accept `csp_nonce`. Under a nonce policy (`style-src 'self' {nonce}`) the browser drops
that element unless it carries the request nonce, and the canvas renders unstyled.

> **Status (2026-11-11): active parity snapshot.** The Dioxus adapter provides modular panels (`FlyContributionSummary`, `FlyPalette`, `FlyLayerTree`, `FlyThemeTokens`, `FlyThemeTokenEditor`, `FlyCanvasPreview`) and `FlyEditorWorkspace` with complete SSR intent and local command execution, design token mutation, contribution filtering, and typed error callbacks matching the `fly-leptos` workspace contract.


`fly-dioxus` is the Dioxus 0.7 component adapter foundation for Fly visual editors.

## Responsibilities

- Render top-level editor shell components (`FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`) using Dioxus `rsx!`.
- Provide `FlyEditorWorkspace`: palette (with contribution-access filtering when the host passes a contribution assembly), layer tree, canvas preview, design-token editor, and undo/redo bound to a `FlyEditor`, with host-resolved `EditorMessages` panel labels; with `on_intent` the workspace instead emits typed `BrowserIntentEnvelope`s (insert, begin palette drag, select, undo, redo, set and remove theme token) for the host's SSR intent endpoint, on the shared `fly-ui` view-models (the parallel twin of the `fly-leptos` workspace). The palette item, its insert and drag controls and the layer row carry the `data-fly-*` attributes of the SSR control contract (`DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md`); the drag control renders in intent mode only.
- Re-export the framework-neutral browser runtime, geometry, and iframe contracts from `fly-web`.

## Known Limitations / Pending Implementation

`FlyEditorWorkspace` is a working editor surface (palette/layers/preview/history) and accepts
a host-owned canvas slot. `FlyCanvasIframe` provides the Dioxus iframe lifecycle: sandboxed
`srcdoc`, exact source-window and origin checks, protocol/instance/sequence validation, and
subscription replacement after reload. The host still owns message interpretation, geometry,
drop policy and persistence. Dioxus-specific pointer/resize gestures and the Page Builder
iframe/drop/resize wiring are separate integration work; they must use `fly-web` geometry and
`fly-ui` commands rather than duplicating editor policy. Dioxus `web`/`desktop` renderers
remain opt-in features.

## Architecture

This crate depends strictly on:
- `fly`: Core document AST and commands.
- `fly-ui`: Visual-editor presentation state machine.
- `fly-web`: Framework-neutral browser runtime, geometry, and iframe bridge.
- `dioxus`: Dioxus 0.7 UI rendering framework.

It contains **zero** Leptos dependencies and **zero** RusTok domain module persistence logic.

## Controlled intent-mode document source of truth

When `on_intent` is provided, the host's `document` prop is authoritative at every render.
The Dioxus workspace's local FlyEditor signal is retained only for standalone mode and must
not shadow a later host document, selection or undo/redo availability. The host may provide
`selected_component_id`, `host_can_undo`, and `host_can_redo` alongside `initial_page` and
`capabilities`. Standalone mode still uses its own live editor document and history.

The Dioxus layer panel is scoped to the same active page index as its preview, through
the canonical `editor_layer_items_for_page` projection. It does not show components from
other pages as if they were selectable in the active canvas; an out-of-range page yields
no layer rows, not an accidental fallback to the first page.
