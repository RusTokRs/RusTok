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
`PageBuilderSanitizedStaticLandingProject` with a SHA-256 hash of the exact sanitized project. This
separates sanitization evidence from runtime materialization without creating a second document
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

For durable page publication, Pages now has a separate consumer-owned atomic service boundary:

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
sanitation-set and artifact-set hashes, the review hash and result version. Exact replay returns the
stored receipt without rebuilding artifacts or emitting duplicate events. Reusing the key for a
different version/body-revision/runtime review fails closed. The selected reviewed scenario/context
must also match the promoted runtime baseline when one exists.

Immutable landing records retain nullable `materialization_hash`, `materialization_identity` and
`runtime_snapshots`. New records require all three and use a five-part key ending in
`materialization_hash`. Legacy records remain readable only with all evidence columns `NULL` and a
valid Fly artifact; partial evidence is rejected. Storefront reads reconstruct and verify the full
materialization envelope before returning HTML.

The reviewed atomic service is integrated, but public Pages GraphQL, HTTP and admin publish
transports still use the legacy publication surface. Their cutover and removal of default-runtime
builder publication are intentionally open rather than represented as completed.

## Machine-readable contracts

- `contracts/page-builder-service-boundary.json` records capability/preview ports and composition.
- `contracts/page-builder-fba-registry.json` records provider/consumer versions and materialization
  persistence.
- `contracts/page-builder-publish-runtime-review.json` records reviewed runtime, sanitizer, Pages
  atomic service, body revision identity, receipt schema, replay semantics and explicit transport
  cutover status.
- `scripts/verify/verify-page-builder-publish-runtime-review.mjs` source-locks those invariants and
  forbids raw runtime-context persistence.

## FFA/FBA status

- **FFA:** `core_transport_ui` for the browser-host slice. `src/browser_host.rs` owns the
  framework-neutral `PageBuilderBrowserModuleDescriptor`; Leptos renders it and future Dioxus must
  consume the same descriptor/DTO/nonce contract.
- **FBA:** `boundary_ready` for preview/materialization and `service_integrated` for the Pages atomic
  reviewed publish core. Public transport cutover, rollback, cache-consumer proof and observed
  rollout evidence remain open.
- **Structural shape:** `core_transport_ui` for browser host and `core_transport` for capability and
  publish contracts.
- **Evidence:**
  - `src/publish_runtime.rs`;
  - `src/publish_sanitization.rs`;
  - `src/static_landing_materialization.rs`;
  - `contracts/page-builder-publish-runtime-review.json`;
  - `crates/rustok-pages/src/dto/page.rs`;
  - `crates/rustok-pages/src/services/page/reviewed_publish.rs`;
  - `crates/rustok-pages/src/entities/page_publish_operation.rs`;
  - `crates/rustok-pages/src/migrations/m20260721_000007_create_page_publish_operations.rs`;
  - `scripts/verify/verify-page-builder-publish-runtime-review.mjs`.

## Open results

1. Cut Pages GraphQL, HTTP and admin publish transports over to `PublishPageInput`. The transport must
   supply exact localized body revisions, a reviewed runtime and an idempotency key, then return the
   durable `PublishPageResult` receipt.
2. Remove builder publication through `PageService::publish_if_current`, legacy default-runtime
   compilation and create-and-publish. Non-builder lifecycle transitions must not become a hidden
   bypass around the reviewed Page Builder contract.
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
