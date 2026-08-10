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

The reviewed path also owns `PageBuilderStaticPublishResourceLimits`. It rejects projects above
16 MiB serialized bytes, 128 pages, 50,000 current component nodes, depth 128, 4,096 assets or
20,000 style rules. The project-byte counter is bounded and component count/depth are observed by an
iterative current-tree scan before recursive policy work. These limits are rechecked during
sanitization integrity and exact materialized compilation. This source is ready; accepted real-project
and runtime execution evidence remains open.

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
or an owner composition root may provide it through Leptos context. The exported
`ConsumerPropertiesPanel` can render in a canvas or standalone host without transferring persistence
ownership. The same contract is intended for a future Dioxus adapter without changing consumer
persistence.

Marker: `admin-provider-status-source-ready`.

The admin FFA now has a typed provider-status seam. `PageBuilderAdminProviderStatus` carries the
exact `BuilderCapabilityFlags` used by a consumer composition plus an optional
`ProviderHealthSnapshot`. Missing health is represented as `unobserved`, never as healthy. Provider
status can only narrow the host's already evaluated tenant/RBAC capabilities: invalid flags,
`builder_off` or observed unavailable health force read-only; degraded health or `publish_off`
disable publish; `properties_off` disables properties; and `preview_off` disables the server-preview
control and its click path. The capability panel shows provider control state, observed health, host
provider policy, rollout flags and observed degradation reasons separately. No fallback editor is
mounted.

`rustok-pages` is the first production contextual consumer. Preview projects the active Fly page,
passes selected runtime context/scenario and rejects late responses when project hash, active page,
context or scenario changed. Pages registers `rustok.pages.metadata` with six typed fields and
provides a port that loads through `fetch_page`, saves through `patch_page_metadata`, binds the command
to `pages:{page_id}:metadata:v{version}`, rejects stale versions and never writes the Fly document.
The canonical panel is mounted in the Fly properties column for draft workspaces and in the
Pages-owned standalone published surface without mounting an editable Fly canvas. The bespoke
`PageMetadataEditor` and its direct workspace persistence path are removed.

Pages exposes the same `pages_builder_capability_flags()` through `PagesBuilderFacade::provider_status`
and through `compose_fly_page_builder_handlers`. The provider-health observation/evaluation/binding
source is now complete: Page Builder records bounded Preview/Publish observations and deployment-
aggregatable metrics/freshness, the exact-target evaluator applies the canonical provider SLO policy,
owner acceptance preserves the remaining `health_valid_until`, and Pages has a fail-closed server
binding plus typed transport into workspace, authoritative SSR, standalone browser-intent and
non-mutating capability preflight. A missing, invalid, expired or uninstalled accepted packet remains
`unobserved`; source inspection never fabricates current deployment health. Configured rollout flags
and observed provider health remain separate inputs, and `effective_provider_runtime_flags` can only
narrow the already-authorized result.

The metadata owner port also exposes a private source-test transport seam. Production still delegates
to the same Pages fetch and patch transports. Focused regressions require the current metadata version
to be rechecked before patch transport, require exact `REVISION_CONFLICT` on stale input with zero
patch calls, and record a metadata-only request while an external dirty Fly sentinel remains unchanged.
These regressions and the corresponding static evidence are source-ready and unvalidated; no executed
packet is claimed.

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
rollback receipt in one transaction. Repair-aware current cursor reconstruction and explicit repeated
physical-loss recovery are source-ready; historical rollback targets still require original manifests
and live immutable artifacts. Rollback reuses immutable artifacts only: it does not call the Page
Builder sanitizer, runtime materializer or compiler. GraphQL, HTTP, OpenAPI, browser retry identity and
the Pages admin prepare/confirm control are connected.

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
artifact-integrity checks. Publish, rollback and explicit repaired-binding activation reuse the same
post-commit `NodePublished` generation rotation. Cache failures fail open to source reads. Accepted
execution evidence remains open.

Authenticated real-DOM authoring, dedicated authoring JS/WASM delivery, same-origin Pages admin launch,
deterministic release composition and anonymous-authoring exclusion are source-ready. They remain
execution/rollout work rather than open source architecture gaps.

Forum is the second production consumer and its Page Builder source path is complete through canonical
module metadata, Fly component/block registration, `ContributionAdapter`, owner preview and owner-backed
property editing. Provider-neutral Page Builder host ports compose Forum only when tenant/module/RBAC
admission succeeds; Forum persistence, visibility, widget schemas, validation and authorization remain
Forum-owned. Exact-source browser, runtime authorization/visibility and deployed server-function
attestation harnesses are retained source. Forum Wave admission is also source-ready and requires those
future execution packets plus `pages_reference_consumer_gate_acceptance_v1` on the same exact source
commit, with deployment-bound packets correlated to the same immutable RepoDigest. The admission step
still does not execute or accept the observed control-plane Wave.

The explicit `pages_reference_consumer_gate` source contract remains fail-closed with `accepted = false`,
`execution_gate = pending` and rollout-candidate provider health `unobserved`. That `unobserved` value
belongs to the rollout-only candidate branch; observed health is a separate exact-source input.
`pages-reference-consumer-gate-acceptance-source.json` is source-ready and requires the rollout candidate
plus `pages_builder_provider_health_observed_acceptance_v1`, explicit owner decision and explicit
rollback disposition. The next Page Builder/Pages/Forum work is maintainer execution of those evidence
and decision packets, not another production-consumer or provider-health architecture slice.

## Machine-readable contracts

- `contracts/page-builder-service-boundary.json` records capability/preview ports and composition.
- `contracts/page-builder-consumer-properties.json` records the framework-neutral property schema,
  port/runtime, Pages owner adapter, independent metadata revision, complete registered UI cutover and
  the source-ready metadata revision/isolation evidence registration.
- `contracts/page-builder-fba-registry.json` records provider/consumer versions, executable consumer
  properties, policy-bound sanitization/materialization persistence, exact publish manifests,
  immutable rollback, repair continuity and the Pages cache consumer boundary.
- `contracts/page-builder-publish-runtime-review.json` records reviewed runtime, the static publish
  policy and sanitizer v2 evidence, Pages atomic publish/rollback services, body revision identity,
  receipt schemas, replay semantics, public transport cutover, explicit ephemeral scenario selection,
  isolated non-builder lifecycle and cache invalidation/read state.
- `contracts/evidence/page-builder-admin-provider-status-source.json` records the admin provider-status
  and degraded-control source boundary without claiming current observed health or execution.
- `contracts/evidence/page-builder-provider-health-runtime-observation-source.json` plus the deployment
  metrics, identity and evaluator source contracts record the bounded observation-to-deployment-health
  architecture without making process-local samples deployment authority.
- `crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json`,
  consumer-binding source, runtime harness source and observed-health acceptance source record the
  fail-closed Pages binding and historical observed-health evidence/owner-decision boundary.
- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json` records the exact
  rollout-only Pages reference-consumer blocker; committed source remains `accepted = false`.
- `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json` records
  the explicit dual-input gate owner/rollback decision source over the rollout candidate and accepted
  observed-health evidence.
- `crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json` records the
  exact-source/deployment correlation required before an observed Forum control-plane Wave may start.
- `scripts/verify/verify-page-builder-admin-provider-status.mjs` source-locks the provider status seam,
  fail-closed capability narrowing, server-preview control and Pages server/UI rollout-flag identity.
- `scripts/verify/verify-page-builder-publish-runtime-review.mjs` source-locks reviewed runtime,
  policy-bound sanitization, exact materialized rechecks and core atomic invariants.
- `scripts/verify/verify-page-builder-publish-transport-cutover.mjs` forbids public legacy/default
  publication and source-locks GraphQL, HTTP, admin reviewed DTO/receipt, scenario-selection and
  non-builder lifecycle boundaries.
- `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate.mjs` source-locks the Pages
  gate identity, profile requirements, owner-read availability, Forum source state and no-live-claim
  boundary.
- `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-acceptance.mjs` source-locks
  exact candidate + observed-health lineage, owner/rollback decisions and non-promotion boundaries.
- `scripts/verify/verify-forum-page-builder-wave-admission.mjs` source-locks accepted-gate consumption,
  Forum evidence lineage and continued observed-Wave pending state.
- `crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs` source-locks exact
  contribution-schema binding, Pages ownership, optimistic metadata revision, registered draft and
  published surfaces, legacy-form absence and the absence of production Fly document writes.
- `crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs` source-locks
  conflict-before-patch ordering, exact stale conflict, metadata-only transport shape, dirty Fly
  isolation regressions and the unvalidated machine evidence boundary.
- `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs` source-locks Pages ownership
  of cache scopes/keys, event-driven invalidation, neutral server capabilities and authorization/cache/
  owner-source ordering in storefront and artifact readers.
- `crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs` source-locks exact publish
  manifests, activation-cursor rollback ordering, immutable-only reuse, typed receipts and public
  transports.

## FFA/FBA status

- **FFA:** `core_transport_ui` for the browser-host slice. Explicit promoted-scenario selection,
  typed rollback control, generation-aware Pages storefront/artifact readers, registered draft and
  published Pages metadata properties, typed admin provider-status/degraded-control seams, and the
  Forum second-consumer host composition are source-connected. Provider-health observation/evaluator/
  binding/consumer source, observed-health runtime harness/owner acceptance, Pages gate acceptance and
  Forum Wave admission are source-ready; their maintainer execution and accepted observed evidence,
  inline edit, anonymous bundle and Forum browser evidence remain open.
- **FBA:** `boundary_ready` for preview, consumer-property contracts and policy-bound
  sanitization/materialization, and `service_and_public_transport_integrated` for Pages reviewed
  publication, immutable rollback/repair continuity and Forum owner-preserving contribution runtime
  source. The default-runtime lifecycle is removed and source-level cache invalidation/read boundaries
  are connected; executed Pages gate/sanitizer/rollback/repair/cache proof, accepted observed-health
  evidence, Forum runtime evidence/admission execution, observed Wave and rollout evidence remain open.
- **Structural shape:** `core_transport_ui` for browser host and `core_transport` for capability,
  properties and publish contracts.
- **Evidence:**
  - `admin/src/provider_status.rs`;
  - `admin/src/transport/mod.rs`;
  - `admin/src/editor/capability_controls.rs`;
  - `admin/src/editor/server_preview.rs`;
  - `admin/src/editor/modular_canvas.rs`;
  - `admin/src/editor/consumer_properties.rs`;
  - `contracts/page-builder-consumer-properties.json`;
  - `src/publish_runtime.rs`;
  - `src/static_publish_policy.rs`;
  - `src/static_publish_resource_limits.rs`;
  - `src/publish_sanitization.rs`;
  - `src/static_landing.rs`;
  - `src/static_landing_materialization.rs`;
  - `contracts/page-builder-publish-runtime-review.json`;
  - `contracts/page-builder-fba-registry.json`;
  - `admin/src/publish_scenario_selection.rs`;
  - `admin/src/editor/publish_scenario_selector.rs`;
  - `crates/rustok-pages/admin/src/builder.rs`;
  - `crates/rustok-pages/admin/src/contributions.rs`;
  - `crates/rustok-pages/admin/src/metadata_properties.rs`;
  - `crates/rustok-pages/admin/src/standalone_metadata.rs`;
  - `crates/rustok-pages/admin/src/lib.rs`;
  - `crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json`;
  - `crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json`;
  - `crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json`;
  - `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json`;
  - `crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json`;
  - `crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json`;
  - `crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json`;
  - `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-acceptance.mjs`;
  - `scripts/verify/verify-forum-page-builder-wave-admission.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs`;
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
  - `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`;
  - `crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs`.

## Open results

1. Run and retain the focused stale metadata revision and dirty Fly isolation packets.
2. Retain a published metadata browser packet proving the registered save advances only metadata
   version while the editable Fly canvas remains unmounted.
3. Retain an accepted sanitizer/resource-limit packet covering unsafe authoring input, global budgets
   and runtime-injected URL/CSS rejection with policy hash, reviewed publish receipt and zero persisted
   artifact/event side effects.
4. Retain accepted publish, rollback and repair cache packets correlating receipt, `NodePublished`,
   handler receipt, generation rotation and storefront/artifact cache miss/refill.
5. Execute the exact provider-health maintainer chain for one deployment: capture exact target identity,
   evaluate deployment metrics/freshness, take the binding-owner decision, install the accepted binding
   packet while its lease is live, run the observed-health consumer harness and take the retrospective
   observed-evidence owner decision. Do not infer current Ready/Degraded/Unavailable state from source.
6. Execute the rollout-only reference candidate, combine it with owner-accepted observed-health evidence,
   and take the explicit Pages gate owner + rollback decision. Committed source remains
   `pages_reference_consumer_gate.accepted = false` until that maintainer-owned packet exists.
7. Execute Forum browser/runtime/server-function evidence on the same exact source/deployment boundary,
   run `forum_page_builder_wave_admission_v1`, then perform the separate observed control-plane Wave with
   audit trail, fallback profiles, metrics/traces, rollback decision, approvals and waivers before owner
   review.
8. Add the first Dioxus host renderer after Dioxus enters the workspace. It must render
   `PageBuilderBrowserModuleDescriptor` and reuse the canonical runtime DTO.
9. Promote FFA/FBA only after observed Pages/Forum evidence and provider-health requirements are met.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-consumer-binding.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-observed-acceptance.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-acceptance.mjs`;
- `node scripts/verify/verify-forum-page-builder-wave-admission.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-static-publish-resource-limits.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate.mjs`;
- `node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs`;
- `node scripts/verify/verify-forum-page-builder-browser-evidence-harness.mjs`;
- `node scripts/verify/verify-forum-page-builder-runtime-authorization-evidence.mjs`;
- `node scripts/verify/verify-forum-page-builder-serverfn-deployment-attestation.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs`;
- `cargo test -p rustok-pages-admin stale_metadata_revision_short_circuits_before_patch_transport`;
- `cargo test -p rustok-pages-admin metadata_save_is_document_free_and_preserves_dirty_fly_state`;
- `node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo test -p rustok-pages --lib`;
- `cargo xtask module validate page_builder`.

These are execution cursors only. They were not run by this source-authoring slice.

## Boundaries

- Fly owns the project domain, runtime materialization and validation/rendering semantics.
- Page Builder owns capability delivery, framework-neutral consumer-property and admin-provider-status
  contracts and adapters, preview/review/sanitization/materialization contracts, canonical health policy,
  process-local observation/metrics source, deployment evaluator contract, authorization, transport
  envelopes, feature profiles and server composition order.
- Pages owns property values, optimistic revisions, persistence, publication lifecycle, exact artifact
  manifests, rollback/repair, receipts, cache scope/key policy and the fail-closed binding of a
  maintainer-accepted deployment-health packet into its consumer surfaces. Missing/expired binding remains
  `unobserved`; Pages does not invent deployment health from process-local samples.
- Forum remains owner of Forum widget configuration/schema/validation, owner data reads, visibility and
  authorization even when Page Builder hosts its contribution surfaces. Forum Wave admission only
  correlates accepted Pages/Forum evidence and does not own the observed Wave result.
- Cache/server infrastructure owns shared connection, byte storage and generation primitives only.
- Host frameworks render or bind module surfaces and do not define provider-local contracts.