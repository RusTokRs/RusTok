# Fly runtime integration

`rustok-page-builder` has one current runtime path. Fly owns project decoding, structural
validation, tree traversal, component lookup, project hashing and runtime-scenario release policy.

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
- `PageBuilderRenderingAdapter` for preview rendering after Fly validation;
- `PageBuilderRuntimeTelemetry` for started, succeeded and failed operation evidence;
- `PageBuilderScenarioBaselineStore` and `RuntimeScenarioReleasePolicy` for optional release gates;
- canonical DTOs and transport envelopes from `rustok-page-builder`.

The service performs these operations:

```text
request
  -> FlyProjectInspection::decode_with
  -> inspection.require_valid
  -> optional runtime-scenario release gate
  -> rendering or persistence port
  -> PageBuilderRuntimeCallEvidence
  -> typed capability result
```

A composition root applies guards outside the service:

```rust
use rustok_page_builder::adapters::FlyAdapterBackedPageBuilderService;
use rustok_page_builder::service::{
    AuthorizedPageBuilderHandlers, CapabilityGuardedService,
};

let service = FlyAdapterBackedPageBuilderService::new(project_store, rendering_adapter);
let service = CapabilityGuardedService::new(service, rollout_flags);
let handlers = AuthorizedPageBuilderHandlers::new(service);
```

GraphQL and Leptos server-function endpoints dispatch through the same authorized handlers and
canonical request/response envelopes. Future Dioxus hosts use the same service and transport
contracts rather than defining a second provider.

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
