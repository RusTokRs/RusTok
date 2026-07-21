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
5. validates the persisted page identity and non-empty revision returned by the store;
6. records `PageBuilderRuntimeCallEvidence` with success only after the persisted result is valid;
7. returns the canonical typed capability response.

The persistence port returns `PageBuilderProjectSaveResult` directly. The result carries the actual
persisted page id, revision id and publication state. `FlyAdapterBackedPageBuilderService` converts
that domain result into `PublishPageBuilderResult`; consumers no longer use shared mutable state or
string parsing to recover persistence output.

The module-owned `compose_fly_page_builder_handlers` entrypoint fixes the server composition order.
It validates rollout flags, wraps the Fly service with `CapabilityGuardedService`, and then creates
`AuthorizedPageBuilderHandlers`. The configured variant accepts telemetry/baseline-enabled Fly
services, explicit port policies and an explicit authorizer without changing that order.

GraphQL and Leptos server-function endpoints delegate through the composed handlers and canonical
transport envelopes.

`rustok-pages` is the first production consumer of both server write and server preview delivery.
`PagesBuilderFacade` reduces `Preview` and `Publish` to one consumer-owned
`dispatch_pages_page_builder_capability` seam. The SSR implementation verifies the backend actor,
builds a tenant-scoped `PortContext` with capability-specific deadline and idempotency, creates the
Pages store and renderer ports, and calls `compose_fly_page_builder_handlers` exactly once. Fly
validation, rollout policy, authorization and port dispatch therefore remain in the module-owned
composition root rather than in capability-specific Pages pipelines.

CSR and hydrate use the single `pages_page_builder_capability` Leptos server function with the
canonical `PageBuilderCapabilityRequest`. The client implementation never calls
`save_page_document`; after a successful publish it performs only a read-back so the existing Pages
UI callback can refresh metadata and revision state. `PagesPageBuilderProjectStore` remains the only
Pages document write port and returns the actual persisted `PageBuilderProjectSaveResult`.

`PagesPageBuilderRenderer` revalidates tenant and actor identity and confirms that the requested Pages
document exists in the tenant before rendering. The Page Builder admin runtime projects only the
active internal Fly page into the preview request, consumes `PreviewPageBuilderResult`, and exposes
the returned HTML in a separate sandboxed read-only iframe. A response is accepted only while the
project hash and active-page index still match the request, so a late server response cannot replace
the current preview with stale HTML. The local instrumented canvas remains the authoring surface and
is not a second server renderer pipeline.

The current machine-readable service contract is
`contracts/page-builder-service-boundary.json`. It records the single Pages capability endpoint and
SSR dispatch helper, composition and authorization order, tenant/page-context guards, client
write-path prohibition and admin preview request identity. It also forbids reference services,
migration decorators, manual JSON preview rendering, capability-specific SSR wrappers and the
removed Pages mutex save-result side channel.

## FFA/FBA status

- FFA status: `core_transport_ui` for the browser-host slice. `src/browser_host.rs` owns the
  framework-neutral `PageBuilderBrowserModuleDescriptor`; the Leptos component only renders its
  script type, adapter marker, optional CSP nonce and source. A future Dioxus renderer consumes the
  same descriptor and nonce contract.
- FBA status: `boundary_ready` with the first production read/write consumer integrated. Fly is the
  domain owner; Page Builder owns capability/port/transport boundaries and server composition order;
  consumer modules own persistence, publication lifecycle and concrete tenant-scoped ports.
- Structural shape: `core_transport_ui` for browser host and `core_transport` for capability service.
- Evidence:
  - `contracts/page-builder-service-boundary.json`;
  - `contracts/page-builder-fba-registry.json`;
  - `src/composition.rs`;
  - `crates/rustok-pages/admin/src/builder.rs`;
  - `crates/rustok-pages/admin/Cargo.toml`;
  - `admin/src/editor/runtime.rs`;
  - `admin/src/editor/server_preview.rs`;
  - `admin/src/editor/modular_canvas.rs`;
  - `scripts/verify/verify-page-builder-adapter-seams.mjs`;
  - `scripts/verify/verify-page-builder-endpoint-adapters.mjs`;
  - `scripts/verify/verify-page-builder-transport-bridge.mjs`;
  - `npm run verify:page-builder:fba:baseline`.

## Open results

1. Connect the next production consumer's concrete tenant-scoped store and preview renderer to
   `compose_fly_page_builder_handlers` (or its configured variant). It must return
   `PageBuilderProjectSaveResult` and follow the Pages reference order: verify backend identity,
   construct canonical auth/context, dispatch handlers, then access tenant ports. Consumer-local
   service/guard pipelines, pre-authorization persistence reads and save-result side channels are
   forbidden.
2. Extend server preview with the selected runtime context and scenario contract. Context must flow
   through a canonical Page Builder DTO/port contract rather than through Pages-local renderer
   arguments, and the same request must be usable by future host frameworks.
3. Add the first Dioxus host renderer after Dioxus is introduced into the workspace. It must render
   the `PageBuilderBrowserModuleDescriptor` returned by `page_builder_browser_module`, including
   its optional CSP nonce, and must not copy lifecycle, form, selection or draft-route policy.
4. Replace synthetic Wave evidence with observed tenant control-plane packets. Wave evidence must
   correlate builder write, Pages publish and storefront read across the required rollout profiles.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo xtask module validate page_builder`.

## Boundaries

- Fly owns the project domain and validation/rendering semantics.
- Page Builder owns capability delivery, ports, authorization, transport envelopes, feature
  profiles, runtime evidence, server composition order and the framework-neutral browser module
  descriptor/host source.
- Consumer modules own persistence and publish lifecycle.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.
