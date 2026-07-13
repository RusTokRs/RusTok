# Fly runtime integration

`rustok-page-builder` now has two Fly integration levels under `adapters`.

## Project inspection

`FlyProjectInspection` is the transport-neutral bridge for one canonical `grapesjs_v1` value. It:

- decodes through `fly::GrapesJsV1Codec` without dropping unknown fields;
- validates through Fly registries and configurable node/depth limits;
- exposes validation diagnostics without converting missing providers into destructive migrations;
- builds the layers tree from the real GrapesJS hierarchy at `pages[].component.components`;
- returns component properties without the child subtree;
- exposes the deterministic Fly project hash.

Use `require_valid()` before preview or publish. Warnings such as an unavailable optional provider remain inspectable and do not delete the node. Structural errors such as duplicate component IDs or resource-limit violations reject the operation.

## Provider services

`FlyValidatedPageBuilderService<S>` decorates an existing `PageBuilderCapabilityService`. It is the low-risk migration path: preview and publish are structurally validated by Fly while the wrapped service keeps its existing tree, properties, storage, rendering and telemetry behavior.

`FlyAdapterBackedPageBuilderService<S, R, T>` is the preferred provider for new composition roots. It uses the existing:

- `PageBuilderProjectStore` persistence port;
- `PageBuilderRenderingAdapter` preview/rendering port;
- `PageBuilderAdapterTelemetry` evidence port;
- canonical DTOs, authorization handlers, rollout guards and transport envelopes.

Unlike the legacy reference adapter, its `tree` capability traverses actual GrapesJS pages and components. Its `properties` capability verifies that the requested node exists in the stored project before accepting the property payload.

A typical composition keeps the existing guard order:

```rust
use rustok_page_builder::adapters::FlyAdapterBackedPageBuilderService;
use rustok_page_builder::service::CapabilityGuardedService;

let provider = FlyAdapterBackedPageBuilderService::new(project_store, rendering_adapter);
let provider = CapabilityGuardedService::new(provider, rollout_flags);
```

Authorization and transport adapters remain outside this provider and continue to call the canonical `AuthorizedPageBuilderHandlers` and dispatch functions.

## Browser adapter progress

`fly-leptos` now includes framework-owned browser primitives for:

- host/iframe/scroll/zoom coordinate conversion;
- vertical and horizontal before/inside/after drop-zone interpretation;
- hit-test candidate ranking without using DOM order as project truth;
- insertion-index correction for `After` drops;
- viewport-edge auto-scroll deltas;
- mouse, touch and pen pointer samples;
- versioned iframe messages with instance isolation and replay rejection;
- deterministic listener/observer cleanup registration.

These primitives do not complete the browser phase by themselves. Actual `web-sys` listener installation, ResizeObserver/MutationObserver ownership, pointer capture, iframe lifecycle orchestration, storefront DOM overlays and browser interaction suites remain separate follow-up work.

## Verification

Run:

```text
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fly-runtime.mjs
cargo test -p fly
cargo test -p fly-ui
cargo test -p fly-leptos
cargo test -p rustok-page-builder
```

The Node guard is intentionally compile-free and verifies repository wiring. It does not replace Rust compilation or browser tests.
