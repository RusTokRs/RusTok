# Implementation Plan for `rustok-page-builder`

## Current state

`rustok-page-builder` exposes one Fly-backed capability service for preview, tree, properties and
publish. `FlyAdapterBackedPageBuilderService` owns the runtime sequence; consumer composition roots
supply `PageBuilderProjectStore` and `PageBuilderPreviewRenderingPort` implementations.

The service:

1. decodes imported project data through `FlyProjectInspection`;
2. validates the Fly document before preview or publish;
3. validates the canonical preview runtime context/scenario contract;
4. evaluates the optional runtime-scenario release gate for publish;
5. invokes the selected preview or persistence port;
6. validates the persisted page identity and non-empty revision returned by the store;
7. records `PageBuilderRuntimeCallEvidence` with success only after the selected port result is valid;
8. returns the canonical typed capability response.

`PreviewPageBuilderInput` owns `PageBuilderPreviewRuntime`, which carries a JSON object context and
an optional normalized scenario id. Runtime context is limited to 256 KiB and scenario identity to
128 bytes before the renderer port is called. `FlyAdapterBackedPageBuilderService` passes the complete
input to `PageBuilderPreviewRenderingPort` only after Fly structural validation and runtime-contract
validation. `PreviewPageBuilderResult` returns the selected scenario identity, allowing hosts to
reject responses that no longer match their current context or scenario. Consumer renderers do not
add provider-local runtime arguments.

The capability contract is `1.1`; `consumer_min_version` remains `1.0`. The compatibility guard
accepts consumers in the inclusive range `1.0..=1.1`. Pages adopts `1.1` because it supplies the new
runtime context/scenario fields. Deferred Forum remains on compatible `1.0`; it is not required to
claim runtime-context support before it consumes that surface.

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
Pages store and preview renderer ports, and calls `compose_fly_page_builder_handlers` exactly once.
Fly validation, rollout policy, authorization and port dispatch therefore remain in the module-owned
composition root rather than in capability-specific Pages pipelines.

CSR and hydrate use the single `pages_page_builder_capability` Leptos server function with the
canonical `PageBuilderCapabilityRequest`. The client implementation never calls
`save_page_document`; after a successful publish it performs only a read-back so the existing Pages
UI callback can refresh metadata and revision state. `PagesPageBuilderProjectStore` remains the only
Pages document write port and returns the actual persisted `PageBuilderProjectSaveResult`.

For preview, the Page Builder admin runtime projects only the active internal Fly page and attaches
the selected runtime context and scenario to `PreviewPageBuilderInput`. `PagesPageBuilderRenderer`
revalidates tenant, actor and page identity, confirms that the requested Pages document exists, and
uses `PageBuilderRenderer::render_runtime_document_html` with the DTO context. The returned HTML is
shown in a separate sandboxed read-only iframe only while project hash, active-page index, runtime
context and scenario identity still match the request. A late response cannot replace the current
preview with stale materialization. The local instrumented canvas remains the authoring surface and
is not a second server renderer pipeline.

The current machine-readable service contract is
`contracts/page-builder-service-boundary.json`. It records the preview runtime DTO, contextual port,
resource limits, Fly validation/port order, the single Pages capability endpoint and SSR dispatch
helper, composition and authorization order, tenant/page-context guards, client write-path
prohibition and admin preview request identity. `contracts/page-builder-fba-registry.json` records
contract 1.1, the 1.0 minimum and the runtime-context registry fields. The guards also forbid
reference services, the removed legacy preview port, migration decorators, manual JSON preview
rendering, capability-specific SSR wrappers and the removed Pages mutex save-result side channel.

## FFA/FBA status

- FFA status: `core_transport_ui` for the browser-host slice. `src/browser_host.rs` owns the
  framework-neutral `PageBuilderBrowserModuleDescriptor`; the Leptos component only renders its
  script type, adapter marker, optional CSP nonce and source. A future Dioxus renderer consumes the
  same descriptor, DTO and nonce contract.
- FBA status: `boundary_ready` with the first production contextual read/write consumer integrated.
  Fly is the domain owner; Page Builder owns capability/port/transport boundaries, preview runtime
  contracts and server composition order; consumer modules own persistence, publication lifecycle
  and concrete tenant-scoped ports.
- Structural shape: `core_transport_ui` for browser host and `core_transport` for capability service.
- Evidence:
  - `contracts/page-builder-service-boundary.json`;
  - `contracts/page-builder-fba-registry.json`;
  - `src/dto.rs`;
  - `src/preview_port.rs`;
  - `src/adapters/fly_service.rs`;
  - `src/composition.rs`;
  - `src/health.rs`;
  - `crates/rustok-pages/admin/src/builder.rs`;
  - `crates/rustok-pages/admin/Cargo.toml`;
  - `admin/src/editor/runtime.rs`;
  - `admin/src/editor/server_preview.rs`;
  - `admin/src/editor/modular_canvas.rs`;
  - `tests/preview_runtime_context.rs`;
  - `scripts/verify/verify-page-builder-preview-runtime-contract.mjs`;
  - `scripts/verify/verify-page-builder-adapter-seams.mjs`;
  - `scripts/verify/verify-page-builder-endpoint-adapters.mjs`;
  - `scripts/verify/verify-page-builder-transport-bridge.mjs`;
  - `npm run verify:page-builder:fba:baseline`.

## Open results

1. Connect the next production consumer's concrete tenant-scoped store and contextual preview
   renderer to `compose_fly_page_builder_handlers` (or its configured variant). It must return
   `PageBuilderProjectSaveResult`, consume the complete `PreviewPageBuilderInput`, and follow the
   Pages reference order: verify backend identity, construct canonical auth/context, dispatch
   handlers, then access tenant ports. Consumer-local service/guard pipelines, preview context
   parameters, pre-authorization persistence reads and save-result side channels are forbidden.
2. Add the first Dioxus host renderer after Dioxus is introduced into the workspace. It must render
   the `PageBuilderBrowserModuleDescriptor` returned by `page_builder_browser_module`, including
   its optional CSP nonce, and use the canonical preview runtime DTO without copying lifecycle, form,
   selection, scenario or draft-route policy.
3. Replace synthetic Wave evidence with observed tenant control-plane packets. Wave evidence must
   correlate builder preview context, Pages publish and storefront read across the required rollout
   profiles.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo xtask module validate page_builder`.

## Boundaries

- Fly owns the project domain, runtime materialization and validation/rendering semantics.
- Page Builder owns capability delivery, preview runtime DTOs, ports, authorization, transport
  envelopes, feature profiles, runtime evidence, server composition order and the framework-neutral
  browser module descriptor/host source.
- Consumer modules own persistence, publish lifecycle and concrete tenant-scoped ports.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.
