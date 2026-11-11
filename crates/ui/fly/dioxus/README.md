# Fly Dioxus

For full public interface and component definitions, see [CRATE_API.md](CRATE_API.md).

> **Status (2026-11-11): active parity snapshot.** The Dioxus adapter provides modular panels (`FlyContributionSummary`, `FlyPalette`, `FlyLayerTree`, `FlyThemeTokens`, `FlyThemeTokenEditor`, `FlyCanvasPreview`) and `FlyEditorWorkspace` with complete SSR intent and local command execution, design token mutation, contribution filtering, and typed error callbacks matching the `fly-leptos` workspace contract.


`fly-dioxus` is the Dioxus 0.7 component adapter foundation for Fly visual editors.

## Responsibilities

- Render top-level editor shell components (`FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`) using Dioxus `rsx!`.
- Provide `FlyEditorWorkspace`: palette (with contribution-access filtering when the host passes a contribution assembly), layer tree, canvas preview, design-token editor, and undo/redo bound to a `FlyEditor`, with host-resolved `EditorMessages` panel labels; with `on_intent` the workspace instead emits typed `BrowserIntentEnvelope`s (insert, begin palette drag, select, undo, redo, set and remove theme token) for the host's SSR intent endpoint, on the shared `fly-ui` view-models (the parallel twin of the `fly-leptos` workspace). The palette item, its insert and drag controls and the layer row carry the `data-fly-*` attributes of the SSR control contract (`DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md`); the drag control renders in intent mode only.
- Re-export the framework-neutral browser runtime, geometry, and iframe contracts from `fly-web`.

## Known Limitations / Pending Implementation

`FlyEditorWorkspace` is a working editor surface (palette/layers/preview/history), but there
is still no iframe orchestration, overlays, pointer capture, resize handles, or drop handling
(the palette can begin a drag in intent mode; nothing in this crate resolves the drop);
those layer on top via `fly-web`. The shell components render an accessible `<section>`
wrapper and the host supplies the children. Dioxus-specific event hooks and
coordinate translation helpers are **not** implemented — `key_stroke_from_event`-style conversions
and pointer sessions live in `fly-web` behind `feature = "wasm-client"` and are consumed by the
host directly. Dioxus `web`/`desktop` renderers are opt-in features and are compiled, but no
Dioxus host consumes this crate yet.

## Architecture

This crate depends strictly on:
- `fly`: Core document AST and commands.
- `fly-ui`: Visual-editor presentation state machine.
- `fly-web`: Framework-neutral browser runtime, geometry, and iframe bridge.
- `dioxus`: Dioxus 0.7 UI rendering framework.

It contains **zero** Leptos dependencies and **zero** RusTok domain module persistence logic.
