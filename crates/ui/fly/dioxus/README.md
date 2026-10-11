# Fly Dioxus

`fly-dioxus` is the Dioxus 0.7 component adapter foundation for Fly visual editors.

## Responsibilities

- Render top-level editor shell components (`FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`) using Dioxus `rsx!`.
- Re-export the framework-neutral browser runtime, geometry, and iframe contracts from `fly-web`.

## Known Limitations / Pending Implementation

This crate is a foundation slice, not an editor. It has no canvas, iframe orchestration, overlays,
pointer capture, resize handles, drag-and-drop, panels, or state wiring; the shells render an
accessible `<section>` wrapper and the host supplies the children. Dioxus-specific event hooks and
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
