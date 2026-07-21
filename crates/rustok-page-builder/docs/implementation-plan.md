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
- missing baselines, empty scenarios and multi-scenario baselines without an explicit UI selection
  fail closed;
- `PageService::create` cannot publish or compile through a default runtime.

The old lifecycle method remains internal pending cleanup, but it is no longer reachable from the
public GraphQL, HTTP or admin Page Builder publication surfaces.

## Machine-readable contracts

- `contracts/page-builder-service-boundary.json` records capability/preview ports and composition.
- `contracts/page-builder-fba-registry.json` records provider/consumer versions and materialization
  persistence.
- `contracts/page-builder-publish-runtime-review.json` records reviewed runtime, sanitizer, Pages
  atomic service, body revision identity, receipt schema, replay semantics and public transport
  cutover.
- `scripts/verify/verify-page-builder-publish-runtime-review.mjs` source-locks core atomic invariants.
- `scripts/verify/verify-page-builder-publish-transport-cutover.mjs` forbids public legacy/default
  publication and source-locks GraphQL, HTTP and admin reviewed DTO/receipt boundaries.

## FFA/FBA status

- **FFA:** `core_transport_ui` for the browser-host slice. The single-scenario admin publication path
  is connected; explicit selection for promoted baselines with multiple scenarios remains open.
- **FBA:** `boundary_ready` for preview/materialization and
  `service_and_public_transport_integrated` for Pages reviewed publication. Rollback,
  cache-consumer proof, executed verification and observed rollout evidence remain open.
- **Structural shape:** `core_transport_ui` for browser host and `core_transport` for capability and
  publish contracts.
- **Evidence:**
  - `src/publish_runtime.rs`;
  - `src/publish_sanitization.rs`;
  - `src/static_landing_materialization.rs`;
  - `contracts/page-builder-publish-runtime-review.json`;
  - `crates/rustok-pages/src/dto/page.rs`;
  - `crates/rustok-pages/src/services/page/reviewed_publish.rs`;
  - `crates/rustok-pages/src/graphql/mutation.rs`;
  - `crates/rustok-pages/src/http.rs`;
  - `crates/rustok-pages/admin/src/transport/graphql_adapter.rs`;
  - `crates/rustok-pages/src/entities/page_publish_operation.rs`;
  - `crates/rustok-pages/src/migrations/m20260721_000007_create_page_publish_operations.rs`;
  - `scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
  - `scripts/verify/verify-page-builder-publish-transport-cutover.mjs`.

## Open results

1. Add explicit Pages admin scenario selection for promoted baselines containing more than one
   runtime scenario. The selected scenario/context must be visible before publish and bound into the
   reviewed hash.
2. Remove the now-publicly-unreachable builder publication branch from
   `PageService::publish_if_current` or split non-builder lifecycle transitions into an explicitly
   non-Page-Builder command.
3. Prove the transactional `NodePublished` outbox consumer invalidates all artifact, route and page
   cache keys, and correlate receipt through storefront read telemetry.
4. Add rollback to a previous immutable artifact set with its own idempotent receipt and outbox
   semantics.
5. Connect the next production consumer's concrete tenant-scoped store and contextual preview
   renderer to the canonical composition root without consumer-local authorization or save-result
   side channels.
6. Add the first Dioxus host renderer after Dioxus enters the workspace. It must render
   `PageBuilderBrowserModuleDescriptor` and reuse the canonical runtime DTO.
7. Replace synthetic Wave evidence with observed tenant packets correlating preview context,
   sanitizer identity, materialization, Pages receipt and storefront read.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo test -p rustok-pages --lib`;
- `cargo xtask module validate page_builder`.

## Boundaries

- Fly owns the project domain, runtime materialization and validation/rendering semantics.
- Page Builder owns capability delivery, preview/review/sanitization/materialization contracts,
  ports, authorization, transport envelopes, feature profiles and server composition order.
- Consumer modules own persistence, publication lifecycle, receipts and concrete tenant-scoped
  ports.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.
