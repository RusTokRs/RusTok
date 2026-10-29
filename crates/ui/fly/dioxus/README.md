# Fly Dioxus

For full public interface and component definitions, see [CRATE_API.md](CRATE_API.md).

> **Status (2026-10-14): parked.** This adapter is frozen at the current parity snapshot (workspace with contribution access, SSR intent dispatch incl. theme intents, host-resolved messages). All editor development proceeds on the Leptos adapter until stabilization; Dioxus parity resumes afterwards in one pass.


`fly-dioxus` is the Dioxus 0.7 component adapter foundation for Fly visual editors.

## Responsibilities

- Render top-level editor shell components (`FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`) using Dioxus `rsx!`.
- Provide `FlyEditorWorkspace`: palette (with contribution-access filtering when the host passes a contribution assembly), layer tree, canvas preview, design-token editor, and undo/redo bound to a `FlyEditor`, with host-resolved `EditorMessages` panel labels; with `on_intent` the workspace instead emits typed `BrowserIntentEnvelope`s (insert/select/undo/redo) for the host's SSR intent endpoint, on the shared `fly-ui` view-models (the parallel twin of the `fly-leptos` workspace).
- Re-export the framework-neutral browser runtime, geometry, and iframe contracts from `fly-web`.

## Known Limitations / Pending Implementation

`FlyEditorWorkspace` is a working editor surface (palette/layers/preview/history), but there
is still no iframe orchestration, overlays, pointer capture, resize handles, or drag-and-drop;
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
