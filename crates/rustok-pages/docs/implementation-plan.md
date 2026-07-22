# Implementation Plan for `rustok-pages`

## Policy: current code only

Pages is under active development. It does not keep compatibility editors,
component mirrors, block tables or migration shims.

Forbidden:

- a JSON/CRUD editor beside Fly;
- the deleted Next/GrapesJS page-builder route;
- `frames[0].component` as a component-tree mirror;
- `PageBlock`, `BlockService`, `page_blocks` or block mutations;
- storefront block fallback rendering;
- UI access to raw transport adapters;
- host-owned Pages persistence, cache-key policy or document policy.

The visual document authority is `pages[].component` stored in the Pages body.

## Mission

`rustok-pages` owns page identity, localized metadata and bodies, slugs, channels,
menus, draft/published lifecycle, immutable landing artifacts, publish/rollback
receipts, route/page/artifact cache namespaces and keys, routes and storefront reads.
Fly/Page Builder owns visual document primitives and capability contracts, not
Pages persistence, cache scope or tenant policy.

## Current implementation

### Domain and persistence

- [x] Pages has independent entities for pages, translations, bodies, channel
  visibility, scenario baselines, immutable landing artifacts and publish receipts.
- [x] `PageBlock`, `BlockService`, block DTOs, relations, GraphQL/REST/OpenAPI
  surfaces and storefront block models are deleted.
- [x] The initial development migration never creates `page_blocks`; no drop or
  compatibility migration is retained.
- [x] `PageService` is split into focused current-only modules instead of one
  block-aware monolith.
- [x] New/current documents use only `pages[].component`.
- [x] Unknown current provider/plugin fields are preserved by the Fly codec.
- [x] Page writes use optimistic page versions and body revisions.
- [x] Builder feature flags and scenario-baseline gates fail with typed errors.
- [x] Static landing records persist Page Builder materialization hash, identity
  and runtime snapshot evidence without storing raw runtime context.
- [x] `page_publish_operations` stores one durable result per
  `(tenant_id, page_id, idempotency_key)` with request, sanitization and artifact
  set hashes; it never stores the reviewed runtime context.
- [x] Every new publish receipt also stores an exact immutable locale-to-artifact
  manifest in `page_publish_operation_artifacts`. The manifest hash must equal the
  receipt `artifact_set_hash` in the same transaction.
- [x] `page_rollback_operations` stores an independent idempotent result per
  `(tenant_id, page_id, idempotency_key)` with source/target artifact set hashes,
  target publish operation and result version.
- [x] `PageService::create` always creates a draft. Create-time compilation,
  default-runtime publication and `NodePublished` emission are removed.
- [x] The mixed `publish` / `publish_if_current` lifecycle is removed.
  `publish_non_builder` and `publish_non_builder_if_current` are explicitly limited
  to pages without GrapesJS/Fly bodies and recheck that invariant inside the
  transaction.
- [x] A Page Builder document sent to the non-builder lifecycle fails with
  `PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED`; it cannot compile artifacts or reach a
  raw publish transition.
- [x] Pages owns a bounded cache contract with `route`, `page` and `artifact`
  scopes. Namespace generations are tenant-wide per scope; concrete keys bind
  generation, tenant/page identity and a bounded SHA-256 variant.

### Admin FFA

- [x] Pages owns the Page Builder consumer facade and transport selection.
- [x] Fly saves reload current page metadata and reject stale body revisions.
- [x] Pages contributes current Fly landing blocks through provider/capability
  policy.
- [x] Admin publication calls the reviewed GraphQL command, gathers every current
  localized body revision, creates a deterministic retry key and consumes the
  durable publish receipt.
- [x] Admin rollback transport fetches the current page version, creates a
  deterministic retry key and consumes the separate rollback receipt.
- [x] `PublishScenarioSelectorPanel` renders the promoted baseline scenarios next
  to the regression panel and reacts to capture/import/clear through one live
  baseline signal.
- [x] Selection is scoped by `page_id + baseline_hash` and browser session storage
  contains only the selected scenario id. A one-scenario baseline is automatic;
  multiple scenarios require an explicit exact selection.
- [x] Missing baseline, empty scenarios, missing selection, stale selection and
  foreign scenario ids fail closed before the reviewed command is built.
- [ ] Add the rollback action to the Pages workspace header using the typed admin
  transport; transport and receipt validation are already connected.
- [ ] Metadata editing still needs a typed metadata-only property contribution.

### Storefront FFA

- [x] Published `grapesjs` documents render through Page Builder storefront.
- [x] Static published landing artifacts have a dedicated sandboxed path.
- [x] Storefront GraphQL/native adapters no longer query or synthesize blocks.
- [x] Bound static artifacts are integrity-checked in the same transaction before
  storefront HTML is returned. New records verify the complete Page Builder
  materialization envelope; legacy records are accepted only with all evidence
  columns `NULL` and a valid Fly artifact.
- [x] The composite storefront response uses a cache key bound to route, page and
  artifact generations plus page slug, requested/fallback locale and channel.
- [x] The artifact HTTP delivery path uses the artifact generation plus page,
  locale, fallback locale and channel; module/channel gating runs before lookup,
  and cache fill happens only after the owner artifact service has verified the
  published binding and materialization evidence.
- [x] Cache/generation/provider failures fail open to the owner source read rather
  than serving a stale key or failing the public request.
- [ ] Accepted evidence must prove publish and rollback events rotate generations,
  causing misses and refills on storefront and artifact delivery paths.
- [ ] Storefront should read only the selected immutable published artifact.
- [ ] Authenticated real-DOM inline editing is not implemented.
- [ ] Anonymous bundle exclusion evidence is not complete.

### Page Builder/FBA

- [x] Capability registry, permissions, typed errors, fallback profiles and
  endpoint adapter seams exist.
- [x] Deterministic Fly landing rendering and SHA-256 artifact identity exist.
- [x] Pages persists immutable landing artifact records and bindings.
- [x] Pages persists runtime materialization identity/snapshots with a composite
  uniqueness key that includes `materialization_hash`; partial evidence is
  rejected and raw runtime context is forbidden.
- [x] The provider exposes an explicit reviewed publish-runtime contract. Pages
  verifies its scenario/context hash against materialization identity.
- [x] Page Builder exposes authoritative static publish sanitization through
  `sanitize_static_landing_project`, including stable ids, structural validation
  and secure public-resource policy before materialization.
- [x] `PageService::publish_reviewed` is one idempotent transaction covering page
  and body locks, feature/baseline gates, sanitization, materialization, immutable
  persistence, binding, page transition, transactional outbox events, receipt and
  exact immutable artifact manifest.
- [x] A replay with the same request hash returns the stored receipt without
  rebuilding artifacts or emitting duplicate events; key reuse with another
  request fails with a typed conflict.
- [x] `PageService::rollback_to_previous` atomically restores the latest distinct
  publish manifest. It verifies current and target immutable artifacts through the
  canonical binder, replaces all locale bindings, advances the page version, emits
  `NodeUpdated`/`NodePublished` and stores a rollback receipt.
- [x] Rollback never invokes sanitizer, runtime materialization or compilation and
  remains available independently of current builder/provider health.
- [x] GraphQL publish requires `PublishGqlPageInput` and returns
  `GqlPublishPageResult`; GraphQL rollback requires `RollbackGqlPageInput` and
  returns `GqlRollbackPageResult`.
- [x] HTTP exposes `POST /api/admin/pages/{id}/publish` and
  `POST /api/admin/pages/{id}/rollback`, and OpenAPI registers both typed receipts.
- [x] Admin GraphQL transport sends reviewed runtime for publish and the current page
  version for rollback, using deterministic independent idempotency namespaces.
- [x] The admin publish transport resolves only the explicitly selected scenario
  from the exact current promoted baseline; baseline changes invalidate the
  selection key.
- [x] Create-and-publish is rejected in the domain, so no public transport can
  revive default-runtime builder publication.
- [x] Non-builder publication is isolated from Page Builder persistence and rejects
  every GrapesJS/Fly body with a stable typed code.
- [x] A module-owned event listener consumes page `NodeUpdated`, `NodePublished`,
  `NodeUnpublished` and `NodeDeleted` events. The neutral server adapter rotates
  owner-declared generations through the process-wide `CacheService`; provider
  failures return handler errors for dispatcher retry.
- [x] The same typed server adapter implements `PagesCacheReadPort`; storefront and
  artifact readers consume the shared generation snapshot and cache backend without
  owning Redis or generation policy.
- [ ] Accepted execution evidence must correlate publish/rollback receipts, outbox
  events, handler receipts, generation changes, cache misses and refills.
- [ ] Observed tenant Wave 0/Wave 1 evidence remains open.

## FFA/FBA status

- **FFA:** `in_progress` — reviewed publication, rollback transport, explicit
  promoted-scenario selection and generation-aware storefront/artifact readers are
  connected; the workspace rollback action, typed metadata properties and inline
  edit mode remain open.
- **FBA:** `in_progress` — reviewed runtime, authoritative sanitizer, immutable
  materialization evidence, idempotent publish and rollback services,
  GraphQL/HTTP/admin transports, default-runtime removal and cache
  invalidation/read boundaries are integrated at source level. Executed rollback
  and cache proof, verification and observed rollout evidence remain open.
- **Structural shape:** `core_transport_ui` with one current document authority.

## Ownership boundaries

- **Pages domain/backend:** identity, translations, slugs, channels, templates,
  menus, revisions, reviewed publish transaction, immutable artifact manifests,
  rollback transaction, non-builder lifecycle, receipts, artifact selection, cache
  scopes/namespaces/keys, redirects, deletion and audit.
- **Pages admin FFA:** list/create/select workspace, metadata property
  contributions, Pages persistence facade, publish/rollback actions,
  promoted-scenario selection and permissions.
- **Pages storefront FFA:** published reads, routing, generation-aware cache readers,
  renderer composition and optional authenticated edit mode.
- **Page Builder admin:** editor behaviour and canonical capability envelope.
- **Fly:** current project model, commands, history, registries, validation,
  deterministic rendering and document hash.
- **Page Builder backend FBA:** capability policy, validation/sanitization ports,
  health, feature flags and rollout mechanics.
- **Cache/server host:** process-wide cache connection, byte storage and generation
  primitive only; it does not define Pages scopes, variants or invalidation causes.
- **Hosts:** route, locale, auth and tenant context only.

## Current document/publication model

```text
GraphQL / HTTP / admin reviewed command
  + page metadata version
  + exact localized body revisions
  + idempotency key
  + promoted baseline hash
  + explicit promoted scenario id
  + transient scenario context
  + reviewed runtime hash
  -> page/body locks
  -> feature and promoted-scenario gates
  -> authoritative sanitization
  -> canonical runtime materialization
  -> deterministic renderer
  -> immutable landing artifacts + materialization evidence
  -> published artifact bindings
  -> published page state
  -> transactional NodeUpdated/NodePublished outbox
  -> durable publish receipt + exact artifact manifest
  -> commit
  -> Pages module event listener
  -> rotate tenant route/page/artifact cache generations
  -> generation-aware storefront/artifact miss and refill

Rollback command
  + expected page version
  + independent idempotency key
  -> published page lock
  -> exact replay/collision check
  -> verify current immutable binding set
  -> select latest distinct publish manifest
  -> verify every target immutable artifact and current Page Builder locale body
  -> replace all published locale bindings
  -> advance published page version/state timestamp
  -> transactional NodeUpdated/NodePublished outbox
  -> durable rollback receipt
  -> commit
  -> existing cache generation rotation

Non-builder command
  -> page metadata version
  -> verify no GrapesJS/Fly body before and inside transaction
  -> metadata/body revision concurrency check
  -> published page state + transactional outbox
```

Invariants:

1. `pages[].component` is the sole component-tree authority.
2. Metadata and document writes never overwrite one another implicitly.
3. Draft saves do not mutate the selected published artifact.
4. Publish rejects stale metadata or any stale localized body revision.
5. Artifact identity includes source, renderer release, registry and policy hashes.
6. Materialization evidence includes context hash, scenario identity and runtime
   snapshot hash; raw context is never stored.
7. Reviewed runtime is valid only when SHA-256 binds format, explicit scenario and
   transient context, and promoted baseline evidence matches that scenario/context.
8. The admin selection is ephemeral, stores only a scenario id and is invalidated
   by a different page id or baseline hash.
9. Authoritative sanitization happens before runtime materialization and is bound
   into the operation through `sanitized_set_hash`.
10. A committed publish or rollback idempotency key is immutable: exact replay
    returns its receipt; different input fails closed.
11. Publish page state, artifact bindings, exact artifact manifest, outbox events
    and receipt commit or roll back together.
12. Rollback page state, complete replacement bindings, outbox events and rollback
    receipt commit or roll back together.
13. Rollback targets only exact publish manifests whose canonical hash still matches
    the durable publish receipt; missing legacy manifests fail closed.
14. Rollback reuses verified immutable artifacts and never sanitizes, materializes or
    compiles the current Fly document.
15. Create never publishes; every Page Builder publication crosses the reviewed
    command.
16. Non-builder publication cannot see, compile, bind or publish a GrapesJS/Fly
    document.
17. Cache invalidation is event-driven: publish and rollback do not call cache
    services inline.
18. Pages owns invalidation causes and cache key shape; the server only supplies
    `CacheNamespaceGenerationStore` and a byte cache capability.
19. Tenant-wide per-scope generations keep trusted local snapshots bounded; page id
    and SHA-256 request variants remain part of concrete keys.
20. A handler acknowledges success only after every owner-requested generation has
    advanced and the receipt matches event/correlation identity. A retry may advance
    a generation more than once, which is safe because old keys remain unreachable.
21. Channel/module authorization runs before every cache lookup.
22. Cache fill follows owner source validation; cache errors fail open to source
    reads and do not authorize or publish data.
23. Missing providers fail visibly and never cause silent deletion.
24. Dynamic widgets persist versioned configuration, not privileged snapshots.
25. Anonymous storefront bundles contain no editor code.
26. No block or shadow-editor fallback exists.

## Completed slice — 2026-07-21

- Removed the block entity/DTO/service/GraphQL/REST/OpenAPI contract and all
  storefront block fallback rendering.
- Split `PageService` into focused current-only modules and retained
  `pages[].component` as the sole visual authority.
- Added immutable static landing artifacts, materialization evidence and strict
  storefront verification with fail-closed partial evidence.
- Added `PageBuilderReviewedPublishRuntime`, binding format, explicit scenario and
  transient context through a SHA-256 review hash.
- Added `sanitize_static_landing_project`, which produces a verified deterministic
  project and SHA-256 sanitization identity before materialization.
- Added `PublishPageInput` with page version, exact locale/body revisions,
  idempotency key and reviewed runtime.
- Added `page_publish_operations` and its unique tenant/page/key receipt boundary.
- Replaced the provisional reviewed domain path with one atomic
  `PageService::publish_reviewed` service. It locks page and bodies, validates
  promoted runtime scenario/context, sanitizes, materializes, persists and binds
  immutable artifacts, updates page state, writes outbox events and inserts the
  receipt in one transaction.
- Added typed errors for review, sanitization, materialization mismatch,
  idempotency collision and receipt integrity.
- Cut GraphQL, HTTP and admin publication over to the reviewed command and receipt.
- Removed create-time default-runtime compilation/publication from the domain.
- Added explicit ephemeral promoted-scenario selection, live baseline wiring and
  transport validation against the exact current baseline.
- Unified publish and unpublish UI transport outcomes through
  `PagePublicationResult` and validate returned page identity/version.
- Removed `publish` / `publish_if_current`, introduced an explicit non-builder-only
  lifecycle and added `PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED` for bypass attempts.
- Updated RBAC, locale, lifecycle and language-agnostic integration contracts to
  create drafts and publish through the correct explicit boundary.

## Completed slice — 2026-07-22

- Added `PageCacheScope::{Route, Page, Artifact}` and owner-defined invalidation
  causes for update, publish, unpublish and delete.
- Added bounded tenant-wide generation namespaces and SHA-256 concrete key variants.
- Added `PageCacheInvalidationEventHandler` with event/correlation-bound receipts.
- Registered the listener through `PagesModule` and a typed
  `PagesCacheInvalidationRuntime` extension.
- Added `PagesCacheReadPort` / `PagesCacheReadRuntime` and a neutral shared server
  adapter over `CacheService`, `CacheNamespaceGenerationStore` and `CacheBackend`.
- Connected the composite storefront response to route/page/artifact generations.
- Connected `/api/pages/{id}/artifact` delivery to the artifact generation while
  preserving module/channel gates, ETag, CSP and public cache-control semantics.
- Added bounded serialization/value guards and fail-open cache diagnostics.
- Added exact immutable publish artifact manifests and a fail-closed after-save
  invariant that binds each manifest to its durable publish receipt.
- Added `PageService::rollback_to_previous`, separate rollback receipts, typed
  errors and full immutable binding replacement without renderer/provider calls.
- Added GraphQL, HTTP, OpenAPI and admin GraphQL transport rollback surfaces with an
  independent deterministic idempotency namespace.
- Added `verify-pages-artifact-rollback.mjs` and synchronized both Page Builder
  machine contracts.
- Source guards and runtime tests were not executed in this slice.

## Next implementation order

### P0 — separate metadata and document writes

- [ ] Finish Pages-owned typed metadata property contributions.
- [ ] Track metadata and document revisions independently in every transport.
- [ ] Add conflict tests proving metadata saves cannot replace a dirty/current Fly
  document and Fly saves cannot revert metadata.

### P0 — atomic artifact publication

- [x] Deterministic renderer and artifact identity.
- [x] Immutable artifact persistence and body bindings.
- [x] Runtime materialization identity/snapshot persistence and storefront
  verification.
- [x] Explicit reviewed publish-runtime/scenario contract.
- [x] Authoritative sanitizer before materialization.
- [x] Idempotent atomic reviewed service: lock -> validate -> sanitize -> materialize
  -> compile -> persist -> bind -> state -> outbox -> receipt + exact manifest.
- [x] Cut GraphQL, HTTP and admin transports over to `PublishPageInput`; remove
  public builder publication through the default runtime and disable
  create-and-publish.
- [x] Add explicit admin scenario selection for multi-scenario baselines.
- [x] Remove the mixed builder lifecycle and split an explicit non-builder-only
  publication command with a stable reviewed-publish-required error.
- [x] Connect page lifecycle events to bounded owner-defined route/page/artifact
  generation rotation.
- [x] Adopt generation-aware keys in the composite storefront response and artifact
  delivery reader.
- [x] Add idempotent rollback to the previous distinct immutable artifact set with a
  separate receipt and transactional outbox semantics.
- [ ] Add the typed rollback action to the Pages workspace header.
- [ ] Retain accepted evidence for publish/rollback outbox event → handler receipt →
  generation rotation → cache miss/refill.
- [ ] Correlate publish/rollback receipt, editor save, page/body revisions, runtime
  review, materialization, invalidation receipt, artifact and storefront read in
  telemetry.
- [ ] Add integrity audit and repair/rebuild commands.

### P1 — complete Page Builder authoring

- [ ] Add Media asset contributions without transferring Media ownership.
- [ ] Integrate rich text only through the dedicated opaque payload/editor seam.
- [ ] Generate admin/storefront contribution registries from module metadata.
- [ ] Filter contributions by tenant, permission, capability, provider health and
  surface.
- [ ] Complete accessibility, keyboard and degraded-state coverage.

### P1 — storefront and routing

- [ ] Serve only the selected immutable published artifact.
- [ ] Add locale fallback, canonical URLs, redirects and route-collision policy.
- [ ] Integrate menus, SEO and channel visibility with generation-aware deterministic
  cache keys.
- [ ] Implement authenticated real-DOM inline editing behind permissions/flags.
- [ ] Prove anonymous SSR/CSR/hydrate bundles exclude authoring code.
- [ ] Prove admin preview, published output and inline edit parity.

### P2 — operations and rollout

- [ ] Audit metadata save, document save, publish, replay, unpublish, rollback and
  delete.
- [ ] Metrics for save/publish/rollback latency, conflicts, sanitizer rejection,
  renderer failure, artifact/receipt integrity, invalidation retries and cache hit
  rate.
- [ ] Run observed internal-tenant Wave 0.
- [ ] Run Wave 1 only after publication/rollback/cache gates pass.
- [ ] Prove rollback for missing/corrupt manifests, artifacts, locale bodies and
  cache invalidation failures.

## Verification

- `cargo test -p rustok-pages --lib`
- `cargo clippy -p rustok-pages --lib -- -D warnings`
- `cargo test -p rustok-pages-admin --lib`
- `cargo check -p rustok-pages-storefront --lib`
- `cargo clippy -p rustok-pages-storefront --lib -- -D warnings`
- `node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs`
- `node scripts/verify/verify-pages-current-only.mjs`
- `node scripts/verify/verify-pages-ui-boundary.mjs`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:fba:baseline`
- `cargo xtask module validate pages`
- migration compatibility and full workspace CI

## Update rules

- Update this plan in every Pages implementation slice.
- Checkboxes require merged source; gates require reproducible executed evidence.
- Contract changes require matching guardrails/tests.
- New dependencies require dependency records.
- Never reintroduce block storage, shadow editors, frame mirrors or duplicate
  document authorities.
