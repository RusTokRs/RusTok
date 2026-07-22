# Implementation Plan for `rustok-page-builder`

## Current state

`rustok-page-builder` exposes one Fly-backed capability service for preview, tree, properties and
publish. `FlyAdapterBackedPageBuilderService` owns capability sequencing; consumer composition roots
supply `PageBuilderProjectStore` and `PageBuilderPreviewRenderingPort` implementations.

The capability service:

1. decodes imported project data through `FlyProjectInspection`;
2. validates the Fly document before preview or provider publish;
3. validates the canonical preview runtime context/scenario contract;
4. evaluates the optional runtime-scenario release gate;
5. invokes the selected preview or persistence port;
6. validates returned page identity and revision;
7. records runtime call evidence only after the selected port result is valid;
8. returns the canonical typed capability response.

`PreviewPageBuilderInput` owns `PageBuilderPreviewRuntime`, which carries a JSON object context and
an optional normalized scenario id. Runtime context is limited to 256 KiB and scenario identity to
128 bytes. Preview returns the selected scenario identity so hosts can reject stale responses.

`PageBuilderReviewedPublishRuntime` is the explicit publish-side review contract. It requires a
normalized scenario id and binds `format + scenario_id + transient context` through SHA-256. The
same validator and resource limits are reused when it becomes `PageBuilderPreviewRuntime`. Any
change after review invalidates the hash. Raw context is deliberately absent from durable artifact
and publish-receipt evidence.

`sanitize_static_landing_project` is the authoritative pre-materialization publish policy. It calls
`StaticLandingCompiler::prepare_document`, decodes and validates the current Fly document, assigns
deterministic stable component ids, checks secure public resources and returns
`PageBuilderSanitizedStaticLandingProject`. Its SHA-256 binds the sanitizer format and exact
sanitized project, separating policy evidence from runtime materialization without a second document
model or renderer.

`compile_materialized_static_landing` provides deterministic runtime-bound compilation. It captures
one Fly `RuntimeScenarioRenderSnapshot` per page, materializes through
`materialize_project_with_runtime_context`, compiles the exact resulting document and rechecks the
public resource policy. `PageBuilderMaterializedStaticLandingArtifact` contains SHA-256 context,
snapshot, build/artifact and final materialization hashes. Snapshot `document_hash` remains Fly's
compact `ProjectHash`, while static page content keeps its independent SHA-256 identity.

The capability contract is `1.1`; `consumer_min_version` remains `1.0`. Pages adopts `1.1` because it
consumes runtime context/scenario fields. Deferred consumers may remain on compatible `1.0` until
they adopt that surface.

The module-owned `compose_fly_page_builder_handlers` entrypoint fixes server composition order:
rollout flags, guarded service, authorization and contextual ports. GraphQL and Leptos capability
endpoints delegate through that composition root.

`rustok-pages` is the first production contextual consumer. Preview projects the active Fly page,
passes selected runtime context/scenario and rejects late responses when project hash, active page,
context or scenario changed.

For durable page publication, Pages owns one atomic service boundary:

```text
PublishPageInput
  -> exact metadata/body revision checks
  -> reviewed runtime and promoted scenario/context check
  -> sanitize_static_landing_project
  -> compile_materialized_static_landing
  -> immutable artifact persistence and bindings
  -> published page state
  -> transactional NodeUpdated/NodePublished outbox
  -> page_publish_operations receipt
  -> commit
```

The durable receipt is unique by `(tenant_id, page_id, idempotency_key)` and stores SHA-256 request,
sanitization-set and artifact-set hashes, the review hash and result version. Exact replay returns the
stored receipt without rebuilding artifacts or emitting duplicate events. Reusing the key for a
different version/body-revision/runtime review fails closed. The selected reviewed scenario/context
must also match the promoted runtime baseline when one exists.

Immutable landing records retain nullable `materialization_hash`, `materialization_identity` and
`runtime_snapshots`. New records require all three and use a five-part key ending in
`materialization_hash`. Legacy records remain readable only with all evidence columns `NULL` and a
valid Fly artifact; partial evidence is rejected. Storefront reads reconstruct and verify the full
materialization envelope before returning HTML.

Pages public publication now crosses the reviewed boundary:

- GraphQL requires `PublishGqlPageInput` and returns `GqlPublishPageResult`;
- HTTP exposes `POST /api/admin/pages/{id}/publish` with
  `PublishPageInput -> PublishPageResult`;
- the Leptos admin GraphQL adapter gathers all localized body revisions, prepares a reviewed runtime,
  generates a deterministic snapshot idempotency key and consumes `PublishPageReceipt`;
- `PublishScenarioSelectorPanel` renders the promoted baseline scenarios next to the regression
  baseline panel and tracks baseline capture/import/clear through one shared reactive signal;
- selection is scoped by `page_id + baseline_hash` and stores only the selected scenario id in browser
  session storage; raw runtime context is never stored;
- a one-scenario baseline is selected automatically; multiple scenarios require an explicit exact
  selection, and a missing, stale or foreign selection fails closed in the Pages transport;
- `PageService::create` cannot publish or compile through a default runtime.

The mixed legacy lifecycle has been removed. Non-builder pages use explicitly named
`publish_non_builder` / `publish_non_builder_if_current`; both check before and inside the transaction
that no GrapesJS/Fly body exists. A builder document receives
`PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED` and cannot reach artifact compilation or a raw lifecycle
transition.

Pages now owns the post-commit cache invalidation contract. `PageCacheInvalidationEventHandler`
consumes page `NodeUpdated`, `NodePublished`, `NodeUnpublished` and `NodeDeleted` events. It asks a
typed port to rotate bounded tenant-wide `route`, `page` and `artifact` namespace generations and
validates an event/correlation-bound receipt before acknowledging success. The server adapter supplies
only the shared `CacheNamespaceGenerationStore`; it does not define scopes or concrete key policy.
Generation-aware storefront readers and accepted runtime miss/refill evidence remain open.

## Machine-readable contracts

- `contracts/page-builder-service-boundary.json` records capability/preview ports and composition.
- `contracts/page-builder-fba-registry.json` records provider/consumer versions and materialization
  persistence.
- `contracts/page-builder-publish-runtime-review.json` records reviewed runtime, sanitizer, Pages
  atomic service, body revision identity, receipt schema, replay semantics, public transport cutover,
  explicit ephemeral scenario selection, isolated non-builder lifecycle and cache generation
  invalidation state.
- `scripts/verify/verify-page-builder-publish-runtime-review.mjs` source-locks core atomic invariants.
- `scripts/verify/verify-page-builder-publish-transport-cutover.mjs` forbids public legacy/default
  publication and source-locks GraphQL, HTTP, admin reviewed DTO/receipt, scenario-selection and
  non-builder lifecycle boundaries.
- `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs` source-locks Pages ownership
  of cache scopes/keys, event-driven invalidation and the neutral server generation adapter.

## FFA/FBA status

- **FFA:** `core_transport_ui` for the browser-host slice. Explicit promoted-scenario selection is
  connected for both single- and multi-scenario baselines; typed metadata properties,
  generation-aware storefront readers and inline edit mode remain open.
- **FBA:** `boundary_ready` for preview/materialization and
  `service_and_public_transport_integrated` for Pages reviewed publication. The default-runtime
  lifecycle is removed and source-level cache generation invalidation is connected; rollback,
  cache-reader execution proof, verification and observed rollout evidence remain open.
- **Structural shape:** `core_transport_ui` for browser host and `core_transport` for capability and
  publish contracts.
- **Evidence:**
  - `src/publish_runtime.rs`;
  - `src/publish_sanitization.rs`;
  - `src/static_landing_materialization.rs`;
  - `contracts/page-builder-publish-runtime-review.json`;
  - `admin/src/publish_scenario_selection.rs`;
  - `admin/src/editor/publish_scenario_selector.rs`;
  - `crates/rustok-pages/src/dto/page.rs`;
  - `crates/rustok-pages/src/services/page/reviewed_publish.rs`;
  - `crates/rustok-pages/src/services/page/lifecycle.rs`;
  - `crates/rustok-pages/src/cache_invalidation.rs`;
  - `apps/server/src/services/pages_cache_invalidation.rs`;
  - `apps/server/src/services/module_event_dispatcher.rs`;
  - `crates/rustok-pages/src/graphql/mutation.rs`;
  - `crates/rustok-pages/src/http.rs`;
  - `crates/rustok-pages/admin/src/transport/graphql_adapter.rs`;
  - `crates/rustok-pages/src/entities/page_publish_operation.rs`;
  - `crates/rustok-pages/src/migrations/m20260721_000007_create_page_publish_operations.rs`;
  - `scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
  - `scripts/verify/verify-page-builder-publish-transport-cutover.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`.

## Open results

1. Adopt the owner `page_cache_key` contract in every route/page/artifact cache reader and retain an
   accepted packet correlating `NodePublished`, handler receipt, generation rotation and storefront
   miss/refill.
2. Add rollback to a previous immutable artifact set with its own idempotent receipt and outbox
   semantics.
3. Connect the next production consumer's concrete tenant-scoped store and contextual preview
   renderer to the canonical composition root without consumer-local authorization or save-result
   side channels.
4. Add the first Dioxus host renderer after Dioxus enters the workspace. It must render
   `PageBuilderBrowserModuleDescriptor` and reuse the canonical runtime DTO.
5. Replace synthetic Wave evidence with observed tenant packets correlating preview context,
   sanitizer identity, materialization, Pages receipt, cache generation and storefront read.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo test -p rustok-pages --lib`;
- `cargo xtask module validate page_builder`.

## Boundaries

- Fly owns the project domain, runtime materialization and validation/rendering semantics.
- Page Builder owns capability delivery, preview/review/sanitization/materialization contracts,
  ports, authorization, transport envelopes, feature profiles and server composition order.
- Consumer modules own persistence, publication lifecycle, receipts, cache scope/key policy and
  concrete tenant-scoped ports.
- Cache/server infrastructure owns shared connection and generation primitives only.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.
