# Fly UI

`fly-ui` contains framework-neutral visual-editor behaviour. It depends only on `fly` and standard
serialization/error crates. It does not contain DOM code, Leptos or Dioxus types, RusTok transport,
routing, tenant loading, RBAC implementation, or module-specific widgets.

The crate defines:

- full, inline, preview, and read-only presentations;
- panel, viewport, toolbar, selection, overlay, and dirty-state models;
- editor intents and effects;
- drag-and-drop candidate and insertion contracts;
- contribution, renderer, and property-editor descriptors;
- capability and policy evaluation;
- a deterministic state machine suitable for mock-adapter tests.
