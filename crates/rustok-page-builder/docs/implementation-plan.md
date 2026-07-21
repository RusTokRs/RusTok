# Implementation Plan for `rustok-page-builder`

## Current state

`rustok-page-builder` exposes one Fly-backed capability service for preview, tree, properties and
publish. `FlyAdapterBackedPageBuilderService` owns the runtime sequence; consumer composition roots
supply `PageBuilderProjectStore` and `PageBuilderRenderingAdapter` implementations.

The service:

1. decodes imported project data through `FlyProjectInspection`;
2. validates the Fly document before preview or publish;
3. evaluates the optional runtime-scenario release gate;
4. invokes the selected rendering or persistence port;
5. records `PageBuilderRuntimeCallEvidence`;
6. returns the canonical typed capability response.

`CapabilityGuardedService` and `AuthorizedPageBuilderHandlers` enforce rollout, port-call policy and
permissions. GraphQL and Leptos server-function endpoints delegate through the same handlers and
transport envelopes.

The current machine-readable service contract is
`contracts/page-builder-service-boundary.json`. It explicitly forbids reference services,
migration decorators and manual JSON preview rendering.

## FFA/FBA status

- FFA status: `core_transport_ui` for the browser-host slice. `src/browser_host.rs` owns the
  framework-neutral `PageBuilderBrowserModuleDescriptor`; the Leptos component only renders its
  script type, adapter marker and source. A future Dioxus renderer consumes the same descriptor.
- FBA status: `boundary_ready`. Fly is the domain owner; Page Builder owns capability/port/transport
  boundaries; consumer modules own persistence and publication lifecycle.
- Structural shape: `core_transport_ui` for browser host and `core_transport` for capability service.
- Evidence:
  - `contracts/page-builder-service-boundary.json`;
  - `contracts/page-builder-fba-registry.json`;
  - `scripts/verify/verify-page-builder-adapter-seams.mjs`;
  - `scripts/verify/verify-page-builder-endpoint-adapters.mjs`;
  - `scripts/verify/verify-page-builder-transport-bridge.mjs`;
  - `npm run verify:page-builder:fba:baseline`.

## Open results

1. Connect a production consumer composition root to concrete tenant-scoped project storage and
   preview rendering ports. Done when preview, save and publish execute through
   `FlyAdapterBackedPageBuilderService`, `CapabilityGuardedService` and
   `AuthorizedPageBuilderHandlers` without another service implementation.
2. Add the first Dioxus host renderer after Dioxus is introduced into the workspace. It must render
   the `PageBuilderBrowserModuleDescriptor` returned by `page_builder_browser_module` and must not
   copy lifecycle, form, selection or draft-route policy.
3. Replace synthetic Wave evidence with observed tenant control-plane packets. Wave evidence must
   correlate builder write, Pages publish and storefront read across the required rollout profiles.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo xtask module validate page_builder`.

## Boundaries

- Fly owns the project domain and validation/rendering semantics.
- Page Builder owns capability delivery, ports, authorization, transport envelopes, feature
  profiles, runtime evidence and the framework-neutral browser module descriptor/host source.
- Consumer modules own persistence and publish lifecycle.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.
