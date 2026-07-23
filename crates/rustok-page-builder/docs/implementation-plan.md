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

`sanitize_static_landing_project` is the authoritative pre-materialization publish boundary. It calls
`StaticLandingCompiler::prepare_document`, decodes and validates the current Fly document, assigns
deterministic stable component ids and applies `PageBuilderStaticPublishPolicy`. The fail-closed
policy rejects renderer-fallback tags/component types, markup-bearing or non-renderable opaque
content, dropped/unsafe attributes, unsafe URL schemes, unsupported or orphaned CSS rules, CSS
`url()`/`@import`/`expression`/legacy behavior tokens, invalid assets and unsafe localized page
metadata URLs. Fly's built-in `link` component remains valid because it renders as `<a>`. The
`PageBuilderSanitizedStaticLandingProject` v2 envelope binds `policy_format + policy_hash + exact
sanitized project` through SHA-256; integrity verification re-decodes and revalidates the project.

`compile_materialized_static_landing` provides deterministic runtime-bound compilation. It captures
one Fly `RuntimeScenarioRenderSnapshot` per page, materializes through
`materialize_project_with_runtime_context`, compiles the exact resulting document and rechecks the
complete static publish policy on that exact materialized document. Runtime-injected attributes,
URLs or CSS therefore cannot bypass the reviewed pre-materialization policy.
`PageBuilderMaterializedStaticLandingArtifact` contains SHA-256 context, snapshot, build/artifact and
final materialization hashes. Snapshot `document_hash` remains Fly's compact `ProjectHash`, while
static page content keeps its independent SHA-256 identity.

The capability contract is `1.1`; `consumer_min_version` remains `1.0`. Pages adopts `1.1` because it
consumes runtime context/scenario fields. Deferred consumers may remain on compatible `1.0` until
they adopt that surface.

The module-owned `compose_fly_page_builder_handlers` entrypoint fixes server composition order:
rollout flags, guarded service, authorization and contextual ports. GraphQL and Leptos capability
endpoints delegate through that composition root.

`ConsumerPropertyEditorSchema`, `ConsumerPropertyEditorPort` and
`ConsumerPropertyEditorRuntime` form the framework-neutral consumer-properties boundary. Page Builder
resolves the exact registered property schema from `ContributionAssemblyResult`, requires byte-for-byte
schema equality with the runtime, loads an optimistic-revision snapshot through the consumer port and
returns only a typed save receipt. The current Leptos panel is an adapter; persistence, transport,
revision semantics and field values remain consumer-owned. A facade may supply the runtime directly,
or an owner composition root may provide it through Leptos context. The same contract is intended for
a future Dioxus adapter without changing consumer persistence.

`rustok-pages` is the first production contextual consumer. Preview projects the active Fly page,
passes selected runtime context/scenario and rejects late responses when project hash, active page,
context or scenario changed. Pages now also registers `rustok.pages.metadata` with six typed fields and
provides a port that loads through `fetch_page`, saves through `patch_page_metadata`, binds the command
to `pages:{page_id}:metadata:v{version}`, rejects stale versions and never writes the Fly document.
The executable panel is mounted in the Fly properties column for draft workspaces. The older bespoke
`PageMetadataEditor` still exists in the Pages composition and must be removed in a separate cutover
that preserves metadata editing for published pages.

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
  -> page_publish_operations receipt + exact immutable artifact manifest
  -> commit
```

The durable receipt is unique by `(tenant_id, page_id, idempotency_key)` and stores SHA-256 request,
sanitization-set and artifact-set hashes, the review hash and result version. The sanitization-set hash
therefore transitively binds the versioned static policy hash for every locale. Exact replay returns the
stored receipt without rebuilding artifacts or emitting duplicate events. Reusing the key for a
different version/body-revision/runtime review fails closed. The selected reviewed scenario/context
must also match the promoted runtime baseline when one exists. Every new receipt snapshots the exact
locale-to-artifact membership in `page_publish_operation_artifacts`; its canonical hash must equal the
receipt artifact-set hash in the same transaction.

Immutable landing records retain nullable `materialization_hash`, `materialization_identity` and
`runtime_snapshots`. New records require all three and use a five-part key ending in
`materialization_hash`. Legacy records remain readable only with all evidence columns `NULL` and a
valid Fly artifact; partial evidence is rejected. Storefront reads reconstruct and verify the full
materialization envelope before returning HTML.

Pages public publication crosses the reviewed boundary:

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

Pages also owns an immutable rollback boundary. `PageService::rollback_to_previous` locks the
published page, verifies the active artifact set, resolves the latest activation receipt by page result
version and follows rollback receipts to their referenced publish operation. It then selects only an
older distinct publish receipt, verifies its immutable manifest, replaces every locale binding,
advances the page version, emits `NodeUpdated` and `NodePublished`, and stores a separate idempotent
rollback receipt in one transaction. A matching artifact hash without a publish/rollback activation
receipt is rejected. Rollback reuses immutable artifacts only: it does not call the Page Builder
sanitizer, runtime materializer or compiler. GraphQL, HTTP, OpenAPI, browser retry identity and the
Pages admin prepare/confirm control are connected.

The mixed legacy lifecycle has been removed. Non-builder pages use explicitly named
`publish_non_builder` / `publish_non_builder_if_current`; both check before and inside the transaction
that no GrapesJS/Fly body exists. A builder document receives
`PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED` and cannot reach artifact compilation or a raw lifecycle
transition.

Pages owns the post-commit cache boundary. `PageCacheInvalidationEventHandler` consumes page
`NodeUpdated`, `NodePublished`, `NodeUnpublished` and `NodeDeleted` events, rotates bounded tenant-wide
`route`, `page` and `artifact` generations and validates an event/correlation-bound receipt before
acknowledging success. `PagesCacheReadRuntime` supplies generation-aware bounded JSON reads. The
composite storefront response binds all three generations; artifact HTTP delivery binds the artifact
generation. Module/channel authorization precedes lookup, and cache fill follows owner source and
artifact-integrity checks. Publish and rollback reuse the same post-commit `NodePublished` generation
rotation. Cache failures fail open to source reads. Accepted execution evidence remains open.

## Machine-readable contracts

- `contracts/page-builder-service-boundary.json` records capability/preview ports and composition.
- `contracts/page-builder-consumer-properties.json` records the framework-neutral property schema,
  port/runtime, Pages owner adapter, independent metadata revision and pending bespoke-form removal.
- `contracts/page-builder-fba-registry.json` records provider/consumer versions, executable consumer
  properties, policy-bound sanitization/materialization persistence, exact publish manifests,
  immutable rollback and the Pages cache consumer boundary.
- `contracts/page-builder-publish-runtime-review.json` records reviewed runtime, the static publish
  policy and sanitizer v2 evidence, Pages atomic publish/rollback services, body revision identity,
  receipt schemas, replay semantics, public transport cutover, explicit ephemeral scenario selection,
  isolated non-builder lifecycle and cache invalidation/read state.
- `scripts/verify/verify-page-builder-publish-runtime-review.mjs` source-locks reviewed runtime,
  policy-bound sanitization, exact materialized rechecks and core atomic invariants.
- `scripts/verify/verify-page-builder-publish-transport-cutover.mjs` forbids public legacy/default
  publication and source-locks GraphQL, HTTP, admin reviewed DTO/receipt, scenario-selection and
  non-builder lifecycle boundaries.
- `crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs` source-locks exact
  contribution-schema binding, Pages ownership, optimistic metadata revision and the absence of Fly
  document writes. Its current contract explicitly reports the bespoke form as pending removal.
- `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs` source-locks Pages ownership
  of cache scopes/keys, event-driven invalidation, neutral server capabilities and authorization/cache/
  owner-source ordering in storefront and artifact readers.
- `crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs` source-locks exact publish
  manifests, activation-cursor rollback ordering, immutable-only reuse, typed receipts and public
  transports.

## FFA/FBA status

- **FFA:** `core_transport_ui` for the browser-host slice. Explicit promoted-scenario selection,
  rollback transport, generation-aware Pages storefront/artifact readers and executable typed Pages
  metadata properties are connected. Removing the bespoke metadata form, preserving the property
  surface for published pages and inline edit mode remain open.
- **FBA:** `boundary_ready` for preview, consumer-property contracts and policy-bound
  sanitization/materialization, and `service_and_public_transport_integrated` for Pages reviewed
  publication and immutable rollback. The default-runtime lifecycle is removed and source-level cache
  invalidation/read boundaries are connected; executed metadata/sanitizer/rollback/cache proof,
  verification and observed rollout evidence remain open.
- **Structural shape:** `core_transport_ui` for browser host and `core_transport` for capability,
  properties and publish contracts.
- **Evidence:**
  - `admin/src/consumer_properties.rs`;
  - `admin/src/editor/consumer_properties.rs`;
  - `admin/src/editor/modular_canvas.rs`;
  - `contracts/page-builder-consumer-properties.json`;
  - `src/publish_runtime.rs`;
  - `src/static_publish_policy.rs`;
  - `src/publish_sanitization.rs`;
  - `src/static_landing.rs`;
  - `src/static_landing_materialization.rs`;
  - `contracts/page-builder-publish-runtime-review.json`;
  - `contracts/page-builder-fba-registry.json`;
  - `admin/src/publish_scenario_selection.rs`;
  - `admin/src/editor/publish_scenario_selector.rs`;
  - `crates/rustok-pages/admin/src/contributions.rs`;
  - `crates/rustok-pages/admin/src/metadata_properties.rs`;
  - `crates/rustok-pages/admin/src/lib.rs`;
  - `crates/rustok-pages/src/dto/page.rs`;
  - `crates/rustok-pages/src/services/page/reviewed_publish.rs`;
  - `crates/rustok-pages/src/services/page/rollback.rs`;
  - `crates/rustok-pages/src/services/page/artifact_set.rs`;
  - `crates/rustok-pages/src/services/page/publish_manifest.rs`;
  - `crates/rustok-pages/src/services/page/lifecycle.rs`;
  - `crates/rustok-pages/src/cache_invalidation.rs`;
  - `crates/rustok-pages/storefront/src/transport/native_server_adapter.rs`;
  - `crates/rustok-pages/src/controllers/mod.rs`;
  - `apps/server/src/services/pages_cache_invalidation.rs`;
  - `apps/server/src/services/module_event_dispatcher.rs`;
  - `crates/rustok-pages/src/graphql/mutation.rs`;
  - `crates/rustok-pages/src/http.rs`;
  - `crates/rustok-pages/admin/src/transport/graphql_adapter.rs`;
  - `crates/rustok-pages/src/entities/page_publish_operation.rs`;
  - `crates/rustok-pages/src/entities/page_publish_operation_artifact.rs`;
  - `crates/rustok-pages/src/entities/page_rollback_operation.rs`;
  - `crates/rustok-pages/src/migrations/m20260722_000009_create_page_rollback_operations.rs`;
  - `scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
  - `scripts/verify/verify-page-builder-publish-transport-cutover.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs`.

## Open results

1. Remove the bespoke Pages `PageMetadataEditor` and render the registered consumer property surface
   for both draft and published metadata without mounting an editable Fly document for published pages.
2. Retain an accepted metadata packet proving independent revision conflicts and that metadata saves
   cannot mutate or replace a dirty Fly document.
3. Retain an accepted sanitizer packet covering unsafe authoring input and runtime-injected URL/CSS
   rejection with policy hash, reviewed publish receipt and zero persisted artifact/event side effects.
4. Retain accepted publish and rollback cache packets correlating receipt, `NodePublished`, handler
   receipt, generation rotation and storefront/artifact cache miss/refill.
5. Connect the next production consumer's concrete tenant-scoped store and contextual preview
   renderer to the canonical composition root without consumer-local authorization or save-result
   side channels.
6. Add the first Dioxus host renderer after Dioxus enters the workspace. It must render
   `PageBuilderBrowserModuleDescriptor` and reuse the canonical runtime DTO.
7. Replace synthetic Wave evidence with observed tenant packets correlating preview context,
   sanitizer identity, materialization, Pages publish/rollback receipts, cache generation and
   storefront read.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo test -p rustok-pages --lib`;
- `cargo xtask module validate page_builder`.

## Boundaries

- Fly owns the project domain, runtime materialization and validation/rendering semantics.
- Page Builder owns capability delivery, framework-neutral consumer-property contracts and adapters,
  preview/review/sanitization/materialization contracts, authorization, transport envelopes, feature
  profiles and server composition order.
- Consumer modules own property values, optimistic revisions, persistence, publication lifecycle,
  exact artifact manifests, rollback, receipts, cache scope/key policy and concrete tenant-scoped
  ports.
- Cache/server infrastructure owns shared connection, byte storage and generation primitives only.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.
