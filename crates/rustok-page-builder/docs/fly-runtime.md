# Fly runtime integration

`rustok-page-builder` has one current runtime path. Fly owns project decoding, structural
validation, tree traversal, component lookup, project hashing, runtime materialization and
runtime-scenario release policy.

## Project inspection

`FlyProjectInspection` is the transport-neutral view of an imported project value. It:

- decodes through `fly::GrapesJsCodec` without dropping unknown fields;
- validates through Fly registries and configurable node/depth limits;
- keeps the decoded `ProjectDocument` and its `ValidationReport` together;
- builds the layers tree from `pages[].component.components`;
- returns component properties without the child subtree;
- exposes the deterministic Fly project hash.

Preview and publish call `require_valid()` before invoking persistence or rendering ports.
Structural errors such as duplicate component IDs or resource-limit violations reject the
operation.

## Current service

`FlyAdapterBackedPageBuilderService<S, R, T, B>` is the only Page Builder capability service
implementation owned by this crate. It uses:

- `PageBuilderProjectStore` for tenant-scoped project persistence;
- `PageBuilderPreviewRenderingPort` for contextual preview rendering after Fly validation;
- `PageBuilderRuntimeTelemetry` for started, succeeded and failed operation evidence;
- `PageBuilderScenarioBaselineStore` and `RuntimeScenarioReleasePolicy` for optional release gates;
- canonical DTOs and transport envelopes from `rustok-page-builder`.

Preview runtime state is part of the canonical DTO rather than a consumer-specific method
signature:

```rust
let input = PreviewPageBuilderInput::new(page_id, project_data).with_runtime(
    PageBuilderPreviewRuntime::new(runtime_context, selected_scenario_id),
);
```

The service requires `runtime.context` to be a JSON object, validates the optional normalized
scenario identity, then passes the complete `PreviewPageBuilderInput` to the rendering port. The
result echoes the scenario identity as `runtime_scenario_id`. A host can therefore compare the
response with its current project, active page, context and scenario before displaying the HTML.

The preview sequence is:

```text
PreviewPageBuilderInput
  -> FlyProjectInspection::decode_with
  -> inspection.require_valid
  -> preview runtime DTO validation
  -> PageBuilderPreviewRenderingPort::render_preview
  -> runtime materialization in Fly renderer
  -> PreviewPageBuilderResult { html, runtime_scenario_id }
```

Publish retains the independent release sequence:

```text
PublishPageBuilderInput
  -> FlyProjectInspection::decode_with
  -> inspection.require_valid
  -> optional runtime-scenario release gate
  -> PageBuilderProjectStore::save_project
  -> persisted-result validation
  -> PublishPageBuilderResult
```

A composition root applies guards outside the service:

```rust
use rustok_page_builder::adapters::FlyAdapterBackedPageBuilderService;
use rustok_page_builder::service::{
    AuthorizedPageBuilderHandlers, CapabilityGuardedService,
};

let service = FlyAdapterBackedPageBuilderService::new(project_store, preview_renderer);
let service = CapabilityGuardedService::new(service, rollout_flags);
let handlers = AuthorizedPageBuilderHandlers::new(service);
```

GraphQL and Leptos server-function endpoints dispatch through the same authorized handlers and
canonical request/response envelopes. Future Dioxus hosts use the same DTO, service and transport
contracts rather than defining a second provider or local runtime-context parameters.

## Browser host boundary

`crate::browser_host` owns the framework-neutral inline module source used by authoring hosts.
Leptos renders that source as a `<script type="module">`; a future Dioxus adapter can render the
same source without copying lifecycle, form, selection or draft-route policy.

## Verification

```text
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fly-runtime.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs
cargo test -p fly
cargo test -p rustok-page-builder
```

The Node guards verify repository wiring and forbidden obsolete symbols. They do not replace Rust
compilation or browser tests.
