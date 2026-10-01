# Fly Dioxus

`fly-dioxus` is the Dioxus 0.6 component adapter foundation for Fly visual editors.

## Responsibilities

- Render top-level editor shell components (`FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`) using Dioxus `rsx!`.
- Re-export the framework-neutral browser runtime, geometry, and iframe contracts from `fly-web`.
- Provide Dioxus-specific event hooks and coordinate translation helpers.

## Architecture

This crate depends strictly on:
- `fly`: Core document AST and commands.
- `fly-ui`: Visual-editor presentation state machine.
- `fly-web`: Framework-neutral browser runtime, geometry, and iframe bridge.
- `dioxus`: Dioxus 0.6 UI rendering framework.

It contains **zero** Leptos dependencies and **zero** RusTok domain module persistence logic.
