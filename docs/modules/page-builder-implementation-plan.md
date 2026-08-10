---
id: doc://docs/modules/page-builder-implementation-plan.md
kind: development_plan
language: en
status: active
---

# Fly Ecosystem and Page Builder Implementation Plan

## Status legend

- `[x]` — implemented in repository source.
- `[ ]` — not implemented or not reproducibly verified.
- Source completion and phase-gate completion are separate: gates require
  executable Rust, WASM, browser and runtime evidence.

## Current-source actualization

This central plan is reconciled to current `main` through PR #3435 on 2026-08-10. The detailed
source overlays under `docs/modules/page-builder-*-actualization-2026-08-07.md`, the 2026-08-08
Pages/Page Builder parity packets and the 2026-08-09/10 provider-health, gate-acceptance, Forum
Wave-admission and base-plan-reconciliation overlays remain authoritative where they are more
specific. Older open checkboxes for Pages metadata contributions, immutable rollback, artifact
audit/repair, reviewed static resource limits, authenticated real-DOM authoring, anonymous authoring
exclusion, generated contribution-registry foundations, shared contribution metadata tooling, the
Forum Fly adapter/owner-preview/property-editor source and the missing provider-health architecture
cursor were stale and are corrected below.

Current source markers for this slice:

```text
Provider status/degraded controls: source-ready
Provider health observation/evaluator/binding/consumer chain: source-ready
Observed-health runtime harness/owner acceptance: source-ready
Pages reference-consumer gate acceptance: source-ready
Pages module-metadata contribution generation: source-ready
Shared module contribution tooling: source-ready
Forum second-consumer contribution discovery: source-ready
Forum Fly adapter/component registry: source-ready
Forum owner preview transport/Pages host composition: source-ready
Forum owner-backed property editing: source-ready
Forum Wave admission: source-ready
```

Observed provider-health **execution** remains a maintainer-owned cursor, but the observation,
deployment aggregation/evaluation, owner acceptance, fail-closed Pages binding and consumer
narrowing source architecture now exists. A missing, invalid, expired or uninstalled accepted health
packet is represented as `unobserved`, not healthy. The rollout-only reference candidate also keeps
`provider_health = unobserved` intentionally because observed health is a separate exact-source gate
input. Source inspection does not assert the current deployment state.

The Pages reference consumer keeps its complete Fly contribution declaration in canonical
`rustok-module.toml`. `rustok-pages-admin/build.rs` delegates generic parsing, provider/version
injection and capability validation to the reusable
`rustok-build/src/module_manifest_contribution.rs` tooling source, while retaining only Pages-specific
role/constant assertions. `xtask module validate` consumes the same normalizer for publish readiness.
Admin/WASM runtime still does not parse TOML and no handwritten Pages `ContributionDescriptor` tree
remains.

Forum is the second production consumer on the shared contribution boundary. Canonical
`rustok-module.toml` declares two complementary owner-provider admin contributions for the existing
`forum.topic_list`, `forum.topic_detail` and `forum.reply_stream` widget contracts.
`rustok.forum.widget-catalog` owns blocks and owner-schema-reference property contracts under
`tree + properties`; `rustok.forum.widget-preview` owns only renderer admission under `preview`.
Forum admin build generation consumes the shared normalizer, registers real Fly component/block
identities and exposes a `ContributionAdapter` without importing Forum persistence or owner services.

Forum owner preview and owner-backed property editing are source-ready. `ForumWidgetPreviewService`
normalizes props through the existing Forum contract, applies Forum visibility/RBAC and executes
bounded owner reads, including true pre-pagination `activity/newest/top` sort semantics. The property
path keeps only schema references in contribution metadata, loads current schema bodies from
`ForumWidgetContractService::catalog`, validates candidate configuration through
`ForumWidgetContractService::validate_props`, and patches only valid object-shaped owner
`normalized_props` through the ordinary Fly command/history path. The owner transports are composed
by provider-neutral Page Builder host ports on the real Pages admin route only when Forum is
tenant-enabled and its manifest permission is effectively granted. The Page Builder package itself
has no Forum dependency. Browser/runtime/server-function evidence source remains unexecuted; the
Forum Wave admission source now correlates those future packets with an accepted Pages gate before a
separate observed control-plane Wave may start.

## Current-only policy

Fly and Page Builder are developed without legacy UI or data-authority branches.
GrapesJS remains a behavioural/import-export reference, not a second runtime
source of truth.

The programme forbids:

- parallel JSON/CRUD editors beside Fly;
- hidden JavaScript document authorities;
- component-tree mirrors such as `pages[].component -> frames[0].component`;
- automatic import of obsolete frame trees;
- consumer block tables retained as fallback authoring models;
- host-owned persistence, transport, cache scopes/keys or widget schemas;
- raw runtime-context persistence in publication evidence;
- editor code in anonymous storefront bundles.

The current component-tree authority is `pages[].component`.

## Stable layer split

- `fly` — framework-neutral current project model, lossless unknown-field codec,
  commands, history, registries, validation, rendering, landing readiness and
  deterministic artifact identity;
- `fly-ui` — framework-neutral editor state, panels, selection, overlays, DnD,
  properties, contribution contracts and capability policy;
- `fly-leptos` — browser/Leptos lifecycle, coordinates, iframe/real-DOM adapters
  and event cleanup;
- `rustok-page-builder/admin` — full-authoring shell, canonical admin FFA facade,
  typed provider status/degraded controls and provider-neutral contribution-host
  composition;
- `rustok-page-builder/storefront` — published rendering and authenticated
  real-DOM editing support;
- consumer admin/storefront packages — document lifecycle, metadata, transport,
  persistence adapters and domain contributions;
- domain contribution owners such as Forum — canonical widget configuration,
  owner data reads, validation, visibility and authorization behind public
  contribution/preview/property contracts;
- `rustok-page-builder` backend — capability policy, validation/sanitization,
  preview/review/materialization contracts, health and rollout controls;
- consumer backend — page/document revisions, immutable artifacts, publish
  transactions, receipts, outbox and cache scope/key ownership;
- platform build tooling — canonical module contribution parsing/normalization and
  publish-readiness validation, never runtime registry or tenant policy;
- cache/host infrastructure — shared connection, byte storage and bounded
  generation primitives, never consumer cache policy.

Fly packages do not choose GraphQL, server functions, tenant policy or consumer
persistence. Rich text remains an external dedicated capability.

## Current repository baseline

### Fly engine

- [x] Current project model and unknown-field preservation.
- [x] Stable ids, commands, history, clipboard fragments and revision hashes.
- [x] Component/property registries and missing-provider diagnostics.
- [x] Framework-neutral rendering and landing-readiness checks.
- [x] Deterministic static landing build identity using source, renderer release,
  registry, render policy and SHA-256 artifact hashes.
- [x] Real GrapesJS browser captures and compatibility fixtures exist.
- [ ] Full current Rust/property/browser suites have not been retained as one
  accepted evidence packet for the latest integration.

### Fly UI/browser layers

- [x] `fly-ui` and `fly-leptos` are separate from RusTok domain modules.
- [x] Isolated iframe projection, source/origin/protocol/instance/sequence checks.
- [x] Geometry, viewport, hover, selection and overlay plumbing.
- [x] Palette, command, DnD, resize, keyboard and browser-intent foundations.
- [x] Authenticated real-DOM adapter and Pages inline consumer path are source-ready.
- [ ] Complete accessibility, nested-scroll, race and accepted browser/resource
  evidence remains open.

### Page Builder provider

- [x] Versioned capability registry, permissions, typed errors and health/fallback
  contracts.
- [x] Framework-neutral endpoint adapter seams.
- [x] Tenant control-plane packet schemas and verification scripts.
- [x] Deterministic landing rendering/publish primitives are available through
  Fly and Pages artifact services.
- [x] Canonical preview runtime DTO validation is shared with deterministic static
  materialization. The provider emits a runtime-bound artifact envelope with
  context/scenario/snapshot hashes and Fly preview/static document parity evidence.
- [x] `PageBuilderReviewedPublishRuntime` requires an explicit normalized scenario
  and binds format, transient context and scenario through SHA-256.
- [x] `sanitize_static_landing_project` provides the authoritative static publish
  pre-materialization seam: current Fly decode/validation, deterministic stable
  ids, secure public-resource policy and SHA-256 sanitization evidence.
- [x] Provider-owned HTML/CSS/URL/attribute/static-resource policy is source-ready
  and rechecked on exact materialized output.
- [x] Global reviewed publish resource limits are source-ready: 16 MiB project,
  128 pages, 50,000 components, depth 128, 4,096 assets and 20,000 style rules.
- [x] Public Page Builder publication has no legacy/default-runtime lifecycle path;
  every builder document crosses the reviewed sanitizer/materialization pipeline.
- [x] Admin provider status/degraded controls are source-ready. Rollout flags and
  optional observed health can only narrow host tenant/RBAC capabilities; missing
  health is `unobserved` and no fallback editor is mounted.
- [x] Bounded process-local Preview/Publish observation, deployment-aggregatable
  metrics/freshness, exact source/deployment identity and expected-target inventory
  source are ready. Process-local samples are not deployment authority.
- [x] Reset-aware exact-target deployment evaluator source applies the canonical
  Preview/Publish latency, sanitize-failure and runtime-error policy with freshness
  and minimum-sample admission.
- [x] Binding-owner acceptance, remaining-freshness `health_valid_until`, fail-closed
  Pages server binding, typed provider-health transport and shared effective runtime
  narrowing are source-ready.
- [x] Workspace, authoritative SSR, standalone browser-intent and non-mutating
  capability preflight consume the same validated provider-health narrowing path.
- [x] Observed-health runtime evidence harness and retrospective owner acceptance
  source are ready without asserting current health or extending the historical lease.
- [ ] Accepted parser/real-project/runtime policy evidence is incomplete.
- [ ] Provider-health live exact-target execution/owner decisions and tenant Wave
  evidence remain incomplete.

### Pages reference consumer

- [x] Pages admin mounts Page Builder through a module-owned facade.
- [x] Pages owns optimistic metadata versions, localized body revisions and
  transport selection.
- [x] Metadata-only patch and document-only save commands are separate.
- [x] Consumer metadata editing uses registered typed property contributions; the
  bespoke metadata editor/direct workspace metadata write are removed.
- [x] Pages contribution identities, version-pinned providers, capabilities,
  blocks, messages and the full metadata property schema are canonical module
  metadata and are build-generated into the admin crate without runtime TOML.
- [x] Generic contribution metadata parsing, provider/version injection and
  capability admission are shared through platform build tooling and module
  publish readiness rather than remaining Pages-local.
- [x] The obsolete parallel JSON/CRUD UI and PageBlock persistence/fallback paths
  are deleted.
- [x] Pages provides one builder-first workspace with list/create/select,
  publish/unpublish and delete operations.
- [x] New/current documents use only `pages[].component`.
- [x] Pages storefront renders current Page Builder documents and selected immutable
  static landing artifacts with integrity checks.
- [x] Pages persists and verifies Page Builder runtime materialization identity and
  snapshots. New immutable records carry complete evidence, legacy all-`NULL`
  records retain Fly integrity verification, and partial evidence fails closed.
- [x] `PublishPageInput` binds the operation to metadata version, every localized
  body revision, one idempotency key and one reviewed runtime hash.
- [x] `PageService::publish_reviewed` owns one transaction from page/body locks and
  feature/scenario gates through sanitization, materialization, immutable staging,
  binding, published state, transactional outbox and durable receipt.
- [x] `page_publish_operations` provides durable replay/collision semantics through
  `(tenant_id, page_id, idempotency_key)` and request/sanitization/artifact hashes.
- [x] The atomic reviewed service rejects an empty Page Builder source set and uses
  one locale-ordered source set for scenario evaluation, sanitization and build.
- [x] GraphQL, HTTP and admin transports use `PublishPageInput` and return the
  durable receipt; create-and-publish is rejected.
- [x] Admin publication provides an explicit promoted-scenario selector scoped by
  `page_id + baseline_hash`; session storage contains only the scenario id and
  stale/foreign selections fail closed.
- [x] The mixed lifecycle/default-runtime branch is removed. Explicit
  `publish_non_builder[_if_current]` rejects GrapesJS/Fly bodies with
  `PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED` before and inside the transaction.
- [x] Immutable rollback, bounded artifact integrity audit, explicit append-only
  rebuild, explicit repaired-binding activation, physical-loss recovery including
  repeated rebuilt-artifact loss, and repair-aware current rollback continuity are
  source-ready. Historical rollback targets remain strict.
- [x] Pages owns route/page/artifact cache scopes and SHA-256 generation-aware key
  shape. A module listener consumes page lifecycle events and a neutral server
  adapter rotates bounded tenant-wide generations through the shared cache
  capability.
- [x] The composite storefront response consumes route/page/artifact generations;
  artifact HTTP delivery consumes artifact generation. Module/channel checks run
  before lookup, verified owner reads precede cache fill and cache failures fail
  open to the source read.
- [x] Authenticated inline authoring, dedicated authoring assets, same-origin admin
  launch, deterministic release composition and anonymous authoring exclusion are
  source-ready.
- [x] Pages facade exposes the same provider rollout flags used by its canonical
  server handler composition and has a fail-closed provider-health binding sourced
  only from a maintainer-accepted exact deployment packet. Missing/invalid/expired
  binding remains `unobserved`; current health is never inferred from source.
- [x] Pages reference-consumer gate acceptance source is ready over the rollout-only
  candidate plus owner-accepted observed-health evidence, exact source/RepoDigest,
  explicit owner decision and explicit rollback disposition. Committed gate source
  remains `accepted = false` until maintainer execution.
- [ ] Accepted evidence must correlate outbox delivery, repair/rollback receipts,
  generation rotation, cache miss/refill, browser authoring, provider-health
  execution and the Pages gate decision.

### Forum second contribution consumer

- [x] Forum owns versioned `topic_list`, `topic_detail` and `reply_stream` widget
  contracts plus catalog/validation HTTP routes guarded by `forum_topics:read`.
- [x] Canonical `rustok-module.toml` declares Forum owner-provider contribution
  metadata through the shared module metadata boundary.
- [x] Build-generated Forum admin contribution metadata registers exact version,
  block/component, renderer and owner-schema-reference property identities without
  runtime TOML or a handwritten descriptor tree.
- [x] `rustok.forum.widget-catalog` and `rustok.forum.widget-preview` split
  `tree + properties` from `preview`, preserving property admission when preview
  is disabled.
- [x] Forum supplies real Fly component/block registration plus a
  `ContributionAdapter` that does not read Forum owner data.
- [x] Forum owner preview normalizes existing widget props, applies Forum
  visibility/RBAC and executes bounded topic/reply owner reads through
  `/api/forum/widgets/preview` and the SSR-only Forum admin transport.
- [x] The provider-neutral Page Builder host merges tenant-enabled Forum
  contributions into the Pages consumer registry, installs the Forum Fly registry,
  resolves effective manifest permissions server-side and exposes explicit bounded
  selected-component preview without persisting owner data into Fly.
- [x] Forum owner-backed property editing resolves the admitted property descriptor,
  loads the current schema through Forum owner catalog transport, validates through
  Forum owner normalization and patches only normalized Fly `props` through the
  ordinary command/history path.
- [x] Forum browser, runtime-authorization and deployed server-function evidence
  harness source is retained without claiming execution.
- [x] Forum Wave admission source is ready and requires accepted Pages gate evidence
  plus those Forum packets on one exact checkout source; deployment-bound packets
  must correlate to the same immutable RepoDigest without upgrading maintainer-
  reviewed identity into cryptographic proof.
- [ ] Execute the Forum evidence packets, Wave admission and separate observed
  control-plane Wave; retain audit, fallback, metrics/traces, rollback, approvals,
  waivers and owner review.

## Target architecture

```text
                       REUSABLE FLY LAYERS

  fly
  current project model, codec, commands, validation, rendering, artifacts
    ^
    |
  fly-ui
  editor state, properties, DnD, contributions, capability policy
    ^
    |
  fly-leptos
  browser lifecycle, iframe and real-DOM adapters
    ^                                      ^
    |                                      |
  page-builder/admin                 page-builder/storefront
  full authoring                     published + inline edit
    ^                                      ^
    |                                      |
  consumer admin facade              consumer storefront facade

                         BACKEND / FBA

  consumer domain (Pages)
    -> metadata version + exact localized body revisions
    -> reviewed runtime scenario/context hash
    -> page/body locks and transactional policy gates
    -> authoritative static sanitization
    -> canonical runtime materialization
    -> deterministic artifact build + snapshot/hash evidence
    -> immutable artifact persistence and bindings
    -> explicit rollback/audit/rebuild/activation recovery receipts
    -> published state + transactional outbox
    -> module-owned route/page/artifact generation rotation
    -> generation-aware storefront/artifact cache reads
    -> accepted deployment-health binding can only narrow configured rollout

  contribution owner (Forum)
    -> topic/reply lifecycle, revisions and visibility remain Forum-owned
    -> versioned widget catalog and props validation remain Forum-owned
    -> canonical contribution metadata is shared-tooling validated
    -> Fly identity/configuration only; no copied owner data
    -> owner preview service/HTTP/native transport reauthorize and read Forum state
    -> owner schema/validation transport returns schema + normalized configuration only
    -> Wave admission consumes accepted Pages gate + exact-source Forum evidence

  application composition root
    -> tenant-enabled contribution extensions
    -> server-resolved effective manifest permissions
    -> provider-neutral Page Builder host context
    -> Pages remains document/persistence consumer

  rustok-page-builder
    -> capability policy / health / rollout
    -> process-local observation + deployment evaluator contracts
    -> provider adapter seams
    -> admin provider status/degraded controls
    -> preview/review/sanitization/materialization identity
```

Hosts are composition roots only. They supply route, locale, auth, tenant context
and neutral shared capabilities; they do not own Fly state, Pages/Forum policy,
persistence or cache-key semantics.

## Dependency rules

```text
fly-ui -> fly
fly-leptos -> fly-ui + fly
rustok-page-builder-admin -> fly-leptos -> fly-ui -> fly
rustok-page-builder-storefront -> fly-leptos/fly-ui/fly as required
consumer admin/storefront -> public Page Builder contracts
consumer backend -> Fly/Page Builder validation and rendering contracts
application host -> consumer UI + optional domain contribution UI
```

Forbidden dependencies:

```text
fly -X-> leptos / dioxus / rustok-*
fly-ui -X-> leptos / dioxus / rustok-*
fly-leptos -X-> rustok-*
page-builder backend -X-> admin/storefront UI packages
page-builder UI -X-> optional domain UI packages directly
consumer facade -X-> host application code
```

Shared facades stored in Leptos owner context must be `Send + Sync`; browser
futures may remain local.

## Current document and compatibility contract

GrapesJS compatibility is tested as import/export behaviour:

```text
GrapesJS getProjectData()
  -> Fly deserialize
  -> Fly inspect/mutate
  -> Fly serialize
  -> GrapesJS loadProjectData()
```

Rules:

- `pages[].component` is the current tree authority;
- unknown current fields/providers/plugin metadata remain lossless;
- obsolete frame component trees are not imported or synchronized;
- missing providers produce diagnostics and preserve opaque data;
- a real capture may record GrapesJS normalization, but no compatibility mirror
  may become a second authority.

## Consumer FFA contract

```text
Page Builder admin
  -> canonical PageBuilderAdminFacade
  -> consumer facade
  -> consumer transport
  -> consumer backend
```

Rules:

- UI never selects raw transport adapters;
- Fly never selects RusTok transport or persistence;
- consumer metadata and document revisions are explicit;
- publish carries metadata version, exact localized body revisions, reviewed
  runtime and an idempotency key;
- acknowledgement returns the durable publish receipt;
- promoted runtime scenario selection is explicit, ephemeral and resolved against
  the exact current baseline before publish;
- widget owner data does not flow through the consumer persistence facade;
- dynamic widgets store versioned configuration only;
- contribution preview data is fetched explicitly through an owner port and is
  never persisted into the Fly document by the generic Page Builder UI;
- owner-backed property forms load schema through the owner port and may write only
  the owner's valid normalized configuration into Fly `props`;
- consumer list/create/lifecycle UI remains consumer-owned;
- provider status may only narrow host tenant/RBAC capabilities;
- missing, invalid or expired provider-health binding remains `unobserved` rather
  than healthy;
- rollout-only candidate evidence remains `unobserved` by design while observed
  health is retained in its separate exact-source owner-acceptance branch;
- no fallback editor is mounted when the provider is unavailable: the surface
  remains typed and read-only.

## Security and operations

- Arbitrary component scripts are disabled.
- HTML, CSS, URLs and attributes require authoritative backend policy.
- Global reviewed project/page/component/depth/asset/style budgets are enforced
  before expensive policy traversal and are revalidated at integrity boundaries.
- Runtime-bound public resource URLs are revalidated on the exact materialized
  document before immutable artifact creation.
- Storefront edit mode requires explicit authentication and authorization.
- Dynamic widgets cannot bypass module RBAC.
- Missing providers never cause silent deletion.
- Browser listeners/observers/subscriptions clean up deterministically.
- Project/history/observer/overlay limits are configurable.
- Anonymous storefront bundles exclude authoring assets.
- Artifact identity and integrity are verified before publication/read.
- Raw runtime context is not persisted in selection, artifact or receipt evidence;
  only scenario identity, snapshots and cryptographic hashes are retained.
- Exact idempotency replay returns the stored receipt without rebuild or duplicate
  outbox events; key reuse with different input fails closed.
- Cache invalidation remains post-commit/event-driven; publish/repair transactions
  never call cache infrastructure inline.
- Consumer-owned scopes rotate through bounded shared generations instead of
  wildcard Redis scans/deletes.
- Public cache keys bind tenant/page/request dimensions through bounded SHA-256
  variants; authorization precedes lookup and verified owner reads precede fill.
- Cache backend failures fail open to the authoritative owner source read.
- Handler receipts preserve source event and correlation identity; provider errors
  remain retryable. A retry may safely advance a generation more than once.
- Provider rollout flags and observed health are separate evidence. Process-local
  samples are not deployment authority; accepted deployment health must pass exact
  target/source/freshness/sample admission and may expire back to `unobserved`.
- Consumer contribution metadata is canonical module metadata; runtime source must
  not retain a parallel handwritten descriptor tree.
- Shared contribution parsing/normalization is platform build tooling and publish
  validation only; it must not become runtime registry, tenant or persistence policy.
- Contribution owner transports reauthorize tenant/actor access even when host
  discovery has already admitted the manifest permission.
- Provider-neutral Page Builder host code may merge descriptors/install registries
  and request schema/validation/preview through neutral ports, but must not import
  domain owners such as Forum or duplicate their schemas/data.

## Implementation phases

### Phase 0 — current-only baseline

- [x] Define Fly layers and dependency rules.
- [x] Keep GrapesJS as behavioural compatibility reference.
- [x] Establish `pages[].component` as current authority.
- [x] Delete Pages parallel JSON/CRUD admin UI.
- [x] Remove frame copy/synchronization helpers from Pages.
- [x] Delete backend/storefront `PageBlock` production paths.
- [x] Add guardrails rejecting deleted UI, frame sync and admin blocks.

**Gate:** repository production source contains no obsolete page block or shadow
editor authority. Source is implemented; accepted executed evidence remains part
of the verification programme.

### Phase 1 — engine and codec

- [x] Project model, commands, history, registries and validation.
- [x] Unknown-field/provider preservation.
- [x] Versioned fragments and deterministic revision state.
- [x] Rendering and deterministic landing artifact identity.
- [ ] Retain a current complete real-capture/property/fuzz evidence packet.

### Phase 2 — framework-neutral editor

- [x] Presentation, panels, viewport, selection and overlays.
- [x] DnD/hit-test, clipboard and conflict contracts.
- [x] Renderer/property-editor/contribution contracts.
- [ ] Complete executable full and inline mock-adapter tests.

### Phase 3 — browser adapters

- [x] Coordinates, event lifecycle, iframe validation and teardown.
- [x] Geometry, hover, selection, DnD, resize and keyboard foundations.
- [x] Authenticated real-DOM storefront overlay/patch adapter source.
- [ ] Retain accessibility, nested-scroll, race, resource-budget and browser
  execution evidence.

### Phase 4 — authoritative validation and sanitization

- [x] Add an explicit provider-owned static publish sanitization envelope and
  SHA-256 identity before runtime materialization.
- [x] Route the reviewed static publish path through current Fly traversal,
  structural validation, deterministic ids and secure public-resource checks.
- [x] Remove non-reviewed/default-runtime builder publication paths.
- [x] Provider-owned HTML/CSS/URL/attribute/static-resource policy source.
- [x] Enforce global size/page/component/depth/assets/styles limits across the
  reviewed publish path.
- [ ] Retain real-project runtime/parser tests and accepted typed policy evidence.

### Phase 5 — consumer write separation

- [x] Add metadata-only patch commands.
- [x] Add document-only save commands with body revision.
- [x] Independently conflict-check metadata and document revisions.
- [x] Move Pages metadata editing into registered typed property contributions.
- [ ] Retain focused metadata conflict/isolation and published-surface browser
  execution evidence.

### Phase 6 — deterministic publication

- [x] Landing renderer and build identity.
- [x] Immutable Pages landing artifact entities/services.
- [x] Canonical runtime materialization envelope, snapshot evidence and
  preview/static exact-document parity checks.
- [x] Pages persists runtime materialization identity/snapshots with
  materialization-aware uniqueness and verifies complete evidence on binding and
  storefront reads; legacy all-`NULL` evidence remains backward-compatible.
- [x] Explicit reviewed runtime/scenario contract without raw-context persistence.
- [x] Authoritative static sanitizer before reviewed materialization.
- [x] Idempotent atomic Pages service and reviewed public/admin transport cutover.
- [x] Isolate non-builder publication behind explicitly named commands that reject
  every GrapesJS/Fly body before and inside the transaction.
- [x] Connect page lifecycle events to consumer-owned bounded route/page/artifact
  generation rotation through a typed cache port and neutral server adapter.
- [x] Adopt generation-aware keys in the composite storefront response and artifact
  HTTP delivery reader.
- [x] Rollback to previous immutable artifacts.
- [x] Bounded immutable artifact integrity audit, explicit rebuild and repaired
  binding activation.
- [x] Physical-loss and repeated rebuilt-artifact recovery with repair-aware
  current rollback reconstruction; historical targets remain strict.
- [ ] Retain accepted database/transport/event/receipt/generation/miss/refill
  execution evidence.

### Phase 7 — Page Builder admin

- [x] Manifest-backed FFA package and full-authoring shell.
- [x] Pages builder-first reference workspace.
- [x] Contribution assembly and capability policy foundations.
- [x] Complete reviewed-runtime scenario selection and deterministic idempotency
  transport UX at source level.
- [x] Provider status/degraded controls source: rollout flags, optional observed
  health, fail-closed capability narrowing, preview gate and explicit unobserved
  state.
- [x] Provider-neutral optional contribution host source: external manifest
  assembly, Fly registry installation, effective-permission admission, explicit
  owner-preview and owner-property ports/panels without domain imports in Page
  Builder.
- [x] Provider-health consumer narrowing source: validated accepted health reaches
  workspace, authoritative SSR, standalone browser-intent and capability preflight
  through the shared effective runtime flag policy.
- [ ] Complete remaining generic typed asset/control surfaces and accessibility
  evidence.
- [ ] Retain executed observed provider-health/degraded browser evidence.

### Phase 8 — storefront

- [x] Current published document/static artifact rendering foundations.
- [x] Render selected immutable published artifacts with integrity verification.
- [x] Verify Page Builder runtime materialization evidence before storefront read.
- [x] Use Pages generation-aware cache keys for storefront response and artifact
  delivery reads.
- [x] Authenticated real-DOM editing, authoring asset delivery and draft/published
  source composition.
- [x] Anonymous default/CSR/hydrate/SSR source profiles exclude authoring assets.
- [ ] Retain visual/accessibility, artifact/HTTP/browser and anonymous-bundle
  execution evidence.

### Phase 9 — generated contribution registries

- [x] Separate admin/storefront factories.
- [x] Generate the Pages reference-consumer contribution manifest from canonical
  module metadata at build time, including version targets and property schema.
- [x] Filter by tenant, permission, capability, policy and health.
- [x] Duplicate, cycle, version and missing-provider diagnostics.
- [x] Generalize canonical contribution metadata parsing/normalization into shared
  platform build tooling and `xtask` module publish validation.
- [x] Onboard Forum as the second production consumer to canonical contribution
  metadata without a consumer-local parser or copied schema authority.
- [x] Connect Forum Fly component/block/renderer/property contracts to runtime
  contribution assembly and Pages host registry composition.
- [x] Connect Forum owner preview transport through a provider-neutral host port
  while preserving Forum validation/visibility/RBAC ownership.
- [x] Connect Forum owner-backed property editor runtime through the same neutral
  host boundary, retaining Forum catalog/validation as schema/config authority.

### Phase 10 — rollout

- [ ] Internal tenant Wave 0 with observed evidence.
- [ ] Pages Wave 1 after accepted publication/cache/rollback/repair/browser gates.
- [x] Provider-health observation/evaluator/binding/consumer source chain.
- [x] Observed-health runtime harness and owner-acceptance source.
- [x] Pages reference-consumer gate acceptance source with explicit owner/rollback decision.
- [x] Forum canonical contribution metadata through shared tooling.
- [x] Forum Fly adapter/component registry and runtime contribution assembly source.
- [x] Forum owner-preview host composition source.
- [x] Forum owner-property host composition source.
- [x] Forum browser/runtime/server-function evidence harness source.
- [x] Forum Wave admission source over accepted Pages gate + exact Forum evidence.
- [ ] Execute exact provider-health, Pages gate and Forum evidence/admission packets.
- [ ] Retain observed Forum control-plane Wave evidence and owner review.
- [ ] Media/Pages reusable sections.
- [ ] Blog, Product, Pricing, Taxonomy and SEO contributions.
- [ ] Additional modules only after renderer/property/cache ownership is proven.

## Immediate implementation order

1. Execute the exact provider-health deployment chain: identity/target inventory,
   deployment metrics/evaluator, binding-owner acceptance and live remaining-lease
   Pages binding, followed by the observed-health runtime harness and retrospective
   owner decision. Source inspection must not substitute for current health.
2. Execute the rollout-only Pages reference candidate and combine it with the
   owner-accepted observed-health packet for the explicit Pages gate owner and
   rollback decision. Committed source remains `accepted = false` until then.
3. Execute Forum browser, runtime-authorization and deployed server-function
   evidence on the same exact source/deployment boundary and run the source-ready
   Forum Wave admission correlation.
4. Perform the separate observed Forum control-plane Wave with audit trail,
   fallback profiles, metrics/traces, rollback decision, approvals and waivers,
   then retain owner review.
5. Retain accepted Pages execution evidence for reviewed publish, rollback/repair,
   cache rotation/miss-refill, metadata isolation and authenticated/anonymous
   authoring boundaries.
6. Complete remaining generic Page Builder asset/accessibility controls and their
   executable browser evidence.
7. Promote no new consumer or FFA/FBA wave until its ownership and retained
   execution evidence satisfy the same canonical composition boundary.

## Verification programme

```text
cargo test -p fly
cargo test -p fly-ui
cargo test -p fly-leptos
cargo test -p rustok-page-builder
cargo test -p rustok-page-builder-admin
cargo test -p rustok-page-builder-storefront
cargo test -p rustok-pages
cargo test -p rustok-pages-admin
cargo test -p rustok-pages-storefront
cargo xtask module validate page_builder
cargo xtask module validate pages
cargo xtask module validate forum
node scripts/verify/verify-pages-ui-boundary.mjs
node --test scripts/verify/verify-pages-ui-boundary.test.mjs
node scripts/verify/verify-fly-admin-browser-runtime.mjs
node scripts/verify/verify-fly-ui-contributions.mjs
node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-consumer-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-observed-acceptance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-acceptance.mjs
node scripts/verify/verify-forum-page-builder-wave-admission.mjs
node scripts/verify/verify-forum-wave-plan-sync.mjs
node scripts/verify/verify-forum-wave-evidence-freshness.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-static-publish-resource-limits.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-scenario-baseline.mjs
node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs
npm run verify:page-builder:fba:baseline
npm run verify:page-builder:consumer:pages
npm run verify:page-builder:consumer:forum
npm run verify:i18n:ui
npm run verify:i18n:contract
cargo deny check
cargo audit
```

These commands are execution cursors only. They were not run by the source-authoring
workflow for this actualization.

Required evidence covers current GrapesJS/Fly round trips, iframe rejection and
cleanup, DnD/keyboard/accessibility, metadata/body revision conflicts,
authoritative sanitization/resource budgets, deterministic artifact and receipt
integrity, preview/static materialization parity, idempotent replay, repair/
rollback continuity, event-driven cache generation rotation and public miss/
refill, anonymous authoring exclusion, exact-target provider health/degradation,
generated module metadata authority, shared publish validation, Forum owner-
preview/property host continuity, accepted Pages gate lineage, Forum admission and
observed tenant rollout.

## Update rules

- This is the central cross-module Fly/Page Builder programme plan.
- Consumer local plans are updated in the same change.
- Checkboxes reflect merged source; gates require executed evidence.
- Contract changes require matching guardrails/tests.
- New dependencies require dependency records.
- Do not reintroduce shadow editors, component mirrors, consumer block fallbacks,
  raw runtime-context persistence or host-owned publication/cache policy.