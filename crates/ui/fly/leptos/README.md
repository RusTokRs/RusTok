# Fly Leptos

`fly-leptos` is the first browser/framework adapter for Fly. This initial slice establishes a clean
Leptos-only dependency boundary, reusable full/inline/preview/read-only shells, browser coordinate
value types, hit-test normalization, and lifecycle cleanup registration.

It intentionally does not yet claim the Phase 3 gate. Iframe orchestration, real-DOM overlays,
auto-scroll, pointer capture, resize handles, full drag-and-drop, and browser interaction suites
remain open in the central implementation plan.
