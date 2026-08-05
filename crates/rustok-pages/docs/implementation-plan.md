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
draft/published lifecycle, immutable landing artifacts, publish/rollback
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
- [x] `PagesRollbackControl` mounts the typed rollback action for selected published
  pages, requires prepare/confirm and refreshes the workspace from the returned
  advancing result version.
- [x] Pages registers the typed `rustok.pages.metadata` contribution and renders the
  canonical consumer-property panel inside draft Fly and in a published-only
  standalone host without mounting an editable published Fly document.
- [x] The bespoke `PageMetadataEditor` and its direct workspace persistence path are
  removed; metadata persistence remains owned by `PagesMetadataPropertyPort`.

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
- [x] The server delivery gate runs the real Pages invalidation handler before
  downstream event transport acceptance and uses stable-event process-bounded
  dedupe so relay retry plus the later module listener cannot rotate twice in one
  process.
- [x] A retained server integration source connects real reviewed publish and
  `OutboxRelay` through the production delivery gate to the registered native
  storefront server function using one production `CacheService` for generations
  and bytes. It retains old-key fill, `NodePublished` all-scope rotation, new-key
  miss/refill and same-generation hit without changing production behavior.
- [x] A production gate PostgreSQL publish/rollback restart harness retains durable
  publish and rollback receipts, real `OutboxRelay`, `TenantGenerationDeliveryGate`
  and `ServerPagesCachePort`. Its post-invalidation downstream failure leaves the
  rollback row pending after generation rotation; process-bounded dedupe prevents a
  second rotation when a new relay instance retries the same event UUID.
- [x] A factory-selected Memory and OutboxLocal profile harness constructs the real
  `build_event_runtime` topology. Memory rotates before synchronous listener
  delivery without writing `sys_events`; OutboxLocal writes a pending row first and
  rotates only inside `OutboxRelay` before listener delivery and acknowledgement.
- [x] A selected immutable published artifact regression uses real reviewed publish,
  exact/fallback owner reads and a persisted draft body mutation. The binding,
  artifact hash and document HTML remain unchanged and the draft-only marker never
  becomes public output.
- [x] An anonymous storefront dependency graph verifier defines six feature-resolved
  `cargo metadata` profiles for Pages default/hydrate/SSR and host CSR/hydrate/SSR.
  It excludes dev-dependencies and fails if any non-dev edge reaches Pages admin,
  Page Builder admin, the admin host, or Fly browser/editor crates.
- [x] The current public host is SSR-only for Pages. A source regression rejects
  executable client bootstrap markers, and an explicit artifact inspector requires
  a concrete built SSR artifact instead of treating a missing client bundle as a
  pass.
- [x] Native and unauthenticated GraphQL public detail and list reads use the same
  tenant fallback chain: requested locale, tenant default locale, then platform
  fallback. The legacy owner list wrapper remains available for platform-only
  callers.
- [ ] Accepted evidence must prove publish and rollback events rotate generations,
  causing misses and refills on storefront and artifact delivery paths through the
  production gate.
- [x] Storefront reads only the selected immutable published artifact; current body
  content is not public render authority and can become public only through a later
  reviewed publish or rollback binding replacement.
- [ ] Authenticated real-DOM inline editing is not implemented.
- [ ] Compiled SSR artifact evidence remains open; CSR/hydrate Pages bundle evidence
  becomes mandatory if a Pages client bootstrap is introduced.

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
  `NodeUnpublished` and `NodeDeleted` events. The neutral server delivery gate runs
  the same handler synchronously before downstream acceptance; the separately
  constructed asynchronous listener provider resolves the same process-bounded
  successful-event set and becomes a no-op for an already handled event UUID.
- [x] The same typed server adapter implements `PagesCacheReadPort`; storefront and
  artifact readers consume the shared generation snapshot and cache backend without
  owning Redis or generation policy.
- [x] The retained production relay/native-route source uses the real Page Builder
  reviewed artifact producer contract and confirms the immutable artifact URL stays
  stable while Pages generations and composite keys rotate.
- [x] The PostgreSQL production-gate source keeps Page Builder ownership unchanged:
  it only correlates durable Pages publish/rollback lifecycle rows with the server
  gate, cache generations, retry state and current-key refill behavior.
- [x] Factory-selected Memory and OutboxLocal delivery retain the same Pages owner
  invalidation policy without moving persistence, review, materialization or
  artifact ownership into Page Builder.
- [x] The selected immutable published artifact regression proves Page Builder owns
  artifact production while Pages body identity and published binding own public
  selection; a persisted draft body mutation cannot replace the selected output.
- [x] The anonymous dependency-graph source keeps the read-only
  `rustok-page-builder-storefront → rustok-page-builder → fly` chain while
  forbidding `rustok-page-builder-admin`, `fly-browser`, `fly-ui` and `fly-leptos`
  in every retained non-dev graph.
- [ ] Accepted execution evidence must correlate publish/rollback receipts, outbox
  events, production-gate receipts, generation changes, cache misses and refills.
- [ ] Observed tenant Wave 0/Wave 1 evidence remains open.

## FFA/FBA status

- **FFA:** `in_progress` — reviewed publication, typed rollback control, explicit
  promoted-scenario selection, registered draft/published metadata surfaces,
  generation-aware storefront/artifact readers, production generation gating,
  gate-to-native-route, PostgreSQL retry, local profile, selected immutable
  artifact, tenant-fallback detail/list parity, anonymous dependency-graph and
  SSR-only host source packets are connected. Executed metadata conflict/isolation,
  inline edit mode and compiled SSR artifact evidence remain open.
- **FBA:** `in_progress` — reviewed runtime, authoritative sanitizer, immutable
  materialization evidence, idempotent publish and rollback services,
  GraphQL/HTTP/admin transports, default-runtime removal and production-gated cache
  invalidation/read boundaries are integrated at source level. Server and owner
  harnesses retain native-route key rotation, PostgreSQL retry semantics,
  Memory/OutboxLocal composition, draft-vs-published artifact isolation and
  storefront graph exclusion, but execution, built artifacts, rollback proof,
  verification and observed rollout evidence remain open.
- **Structural shape:** `core_transport_ui` with one current document authority.

## Ownership boundaries

- **Pages domain/backend:** identity, translations, slugs, channels, templates,
  revisions, reviewed publish transaction, immutable artifact manifests,
  rollback transaction, non-builder lifecycle, receipts, artifact selection, cache
  scopes/namespaces/keys, redirects, deletion and audit.
- **Navigation domain/backend:** menu and menu-item identity, localized copy,
  channel/location bindings and public navigation composition.
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
- **Cache/server host:** process-wide cache connection, byte storage, generation
  primitive and neutral synchronous delivery-gate composition. It does not define
  Pages scopes, variants or invalidation causes.
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
  -> Memory: synchronous production gate -> listener delivery
  -> durable profiles: row -> relay -> production gate
  -> event/correlation-bound generation receipt
  -> downstream transport acceptance
  -> asynchronous Pages module listener duplicate no-op
  -> registered generation-aware storefront/artifact miss and refill

Current document save after publication
  -> page_bodies.content and revision advance
  -> selected published artifact binding is unchanged
  -> exact/fallback public reads follow binding artifact_id
  -> current body content is not public render authority

Public localized read
  -> requested locale
  -> tenant default locale when supplied by the public transport
  -> platform fallback locale
  -> selected detail and public list use the same owner resolver

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
  -> same production generation gate and downstream delivery
  -> downstream failure after rotation may retry without a second process-local bump

Non-builder command
  -> page metadata version
  -> verify no GrapesJS/Fly body before and inside transaction
  -> metadata/body revision concurrency check
  -> published page state + transactional outbox
```

Invariants:

1. `pages[].component` is the sole component-tree authority.
2. Metadata and document writes never overwrite one another implicitly.
3. Draft saves do not mutate the selected published artifact; current body content is not public render authority.
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
18. Pages owns invalidation causes and cache key shape; the server supplies
    `CacheNamespaceGenerationStore`, byte cache capability and neutral delivery
    composition.
19. Tenant-wide per-scope generations keep trusted local snapshots bounded; page id
    and SHA-256 request variants remain part of concrete keys.
20. The production delivery gate cannot accept a Pages lifecycle event downstream
    until every owner-requested generation has advanced and the receipt matches
    event/correlation identity. Same-event work is serialized and a successful
    process-local rotation is not repeated by relay retry or the asynchronous
    listener; replay after process restart may conservatively rotate again.
21. Memory applies the gate before in-memory listener delivery and writes no durable
    outbox row; OutboxLocal applies the same gate in the relay target before durable
    acknowledgement.
22. Channel/module authorization runs before every cache lookup.
23. Cache fill follows owner source validation; cache errors fail open to source
    reads and do not authorize or publish data.
24. A route request after generation rotation derives a new composite key; the old
    key may remain physically stored but is unreachable from the current snapshot.
25. Exact and fallback public artifact reads follow the locale body's immutable
    published binding, not the mutable body content.
26. Public detail and list reads use the same tenant fallback chain when the host
    supplies tenant locale policy.
27. Missing providers fail visibly and never cause silent deletion.
28. Dynamic widgets persist versioned configuration, not privileged snapshots.
29. Feature-resolved anonymous storefront graphs exclude admin and Fly authoring
    packages through non-dev dependencies; built SSR artifact proof remains required.
30. No block or shadow-editor fallback exists.

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

## Completed slice — 2026-08-03

- Mounted the typed Pages rollback prepare/confirm control for published pages.
- Registered six typed metadata fields and one Pages-owned optimistic metadata
  owner port.
- Exported and reused the canonical consumer-property panel for draft and published
  Pages lifecycle surfaces.
- Removed the bespoke metadata editor and its direct workspace persistence path.
- Added private source-test transport injection, exact conflict-before-patch ordering
  and focused stale-revision / dirty-Fly isolation regressions.
- Added machine-readable source evidence and static guards while retaining empty
  execution packets and false validation flags.
- Tests, verifiers, formatters, Cargo commands, browser scenarios, workflows and CI
  were not executed in this slice.

## Completed slice — 2026-08-05

- Retained the registered native storefront route, routed-channel admission,
  reviewed immutable artifact selection and one-process relay/refill source packets.
- Rechecked the production relay topology and corrected the synchronous test-target
  evidence so it no longer claimed asynchronous listener completion before outbox
  acknowledgement.
- Extended `TenantGenerationDeliveryGate` to run the real Pages handler before
  downstream transport acceptance under `mod-pages`.
- Added process-bounded stable-event serialization and successful-invalidation
  dedupe to `ServerPagesCachePort`.
- Preserved relay retry after downstream rejection without another generation bump
  and made the later asynchronous Pages listener a same-event rotation no-op.
- Kept supported downstream delivery and module-owned listener registration unchanged.
- Added the server integration source that crosses real `OutboxRelay`, production
  gate and production `ServerPagesCachePort` into the registered native storefront
  server function, retaining old-key fill, all-scope rotation, new-key refill and
  hit behavior.
- Added the production gate PostgreSQL publish/rollback restart harness. It retains
  durable receipt/event commits, a post-invalidation downstream failure, pending
  retry state, a second relay identity, no second process-local generation bump,
  final acknowledgement and ordinary listener duplicate no-op.
- Added the factory-selected Memory/OutboxLocal profile harness. It retains Memory
  synchronous rotation without an outbox row and OutboxLocal pending persistence
  before relay-gated rotation, listener delivery and durable acknowledgement.
- Added the selected immutable published artifact regression: exact and fallback
  reads retain the same binding/hash/HTML across a persisted draft body mutation.
- Added the anonymous storefront dependency graph verifier for six feature-resolved
  profiles, excluding dev-dependencies and forbidding admin/Fly authoring packages.
- Retained the actual SSR-only Pages host boundary and explicit built-artifact
  inspector without claiming a nonexistent client bundle.
- Added fallback-aware public list resolution so native and GraphQL public detail
  and list reads share the tenant default locale before platform fallback.
- Added source evidence, static verifiers and dated production/owner packets.
- Tests, verifiers, formatters, Cargo commands, databases, runtime profiles,
  built artifacts, workflows and CI were not executed in this slice.

## Next implementation order

### P0 — separate metadata and document writes

- [x] Finish Pages-owned typed metadata property contributions.
- [x] Track metadata and document revisions independently in source transports.
- [x] Add source regressions proving stale metadata saves stop before patch transport
  and a metadata-only save cannot mutate an unsaved dirty Fly sentinel.
- [ ] Retain executed conflict and dirty-Fly isolation packets from the focused tests
  and static verifier.
- [ ] Retain a browser packet proving published metadata saves advance only metadata
  version while the editable Fly canvas remains unmounted.

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
- [x] Add the typed rollback action to the Pages workspace header.
- [x] Insert the real Pages handler into the production delivery gate before
  downstream acceptance and deduplicate retry/listener replay by event UUID in one
  process.
- [x] Retain source continuity for reviewed publish → real outbox relay → production
  generation gate → registered native route old-key/new-key behavior.
- [x] Retain source continuity for PostgreSQL publish/rollback receipts → real relay
  → production gate → post-invalidation retry without a second process-local bump.
- [x] Retain factory-selected source continuity for Memory synchronous delivery and
  OutboxLocal durable relay delivery through the same Pages generation gate.
- [x] Retain exact/fallback owner reads across a persisted draft body mutation and
  prove the selected immutable binding remains public authority.
- [ ] Retain accepted execution evidence for publish/rollback outbox event →
  production gate receipt → generation rotation → cache miss/refill.
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

- [x] Serve only the selected immutable published artifact.
- [x] Apply tenant default locale fallback consistently to public detail and list
  reads in native and GraphQL transports.
- [ ] Add canonical URLs, redirects and route-collision policy. The canonical URLs,
  redirects and route-collision policy remain open as a separate routing slice.
- [ ] Compose Navigation-owned menus, SEO and channel visibility with
  generation-aware deterministic cache keys.
- [ ] Implement authenticated real-DOM inline editing behind permissions/flags.
- [x] Retain the anonymous storefront dependency graph verifier across Pages
  default/hydrate/SSR and host CSR/hydrate/SSR profiles.
- [x] Retain the current SSR-only host source boundary and explicit artifact
  inspector contract.
- [ ] Retain compiled SSR artifact evidence proving authoring code and packages
  remain absent. Reopen real CSR/hydrate bundle proof if Pages client delivery is
  introduced.
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

- Contract tests cover every public use case.
- `cargo test -p rustok-pages --lib`
- `cargo clippy -p rustok-pages --lib -- -D warnings`
- `cargo test -p rustok-pages-admin --lib`
- `cargo check -p rustok-pages-storefront --lib`
- `cargo clippy -p rustok-pages-storefront --lib -- -D warnings`
- `node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs`
- `cargo test -p rustok-pages-admin stale_metadata_revision_short_circuits_before_patch_transport`
- `cargo test -p rustok-pages-admin metadata_save_is_document_free_and_preserves_dirty_fly_state`
- `node crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs`
- `cargo test -p rustok-pages --test page_locale_fallback -- --nocapture`
- `node crates/rustok-pages/scripts/verify/verify-pages-selected-immutable-artifact.mjs`
- `cargo test -p rustok-pages --test selected_immutable_published_artifact_sqlite -- --nocapture`
- `node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs`
- `node crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs --artifact <built-ssr-artifact> --output <packet.json>`
- `node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-event-delivery-profile-parity.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs`
- `node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs`
- `cargo test -p rustok-server --features mod-pages --test pages_event_delivery_profiles_sqlite -- --nocapture`
- `cargo test -p rustok-server --features mod-pages --test pages_production_relay_native_route_sqlite -- --nocapture`
- `cargo test -p rustok-server --features mod-pages --test pages_production_gate_postgres_restart -- --nocapture`
- `cargo test -p rustok-server --features mod-pages services::pages_cache_invalidation -- --nocapture`
- `cargo test -p rustok-server --features mod-pages services::tenant_generation_delivery_gate -- --nocapture`
- `cargo test -p rustok-pages --test publish_rollback_outbox_cache_postgres -- --nocapture`
- `cargo test -p rustok-pages --test outbox_relay_restart_postgres -- --nocapture`
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
