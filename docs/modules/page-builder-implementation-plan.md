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
- host-owned persistence, transport or widget schemas;
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
- `rustok-page-builder/admin` — full-authoring shell and canonical admin FFA
  facade;
- `rustok-page-builder/storefront` — published rendering and future authenticated
  real-DOM editing surface;
- consumer admin/storefront packages — document lifecycle, metadata, transport,
  persistence adapters and domain contributions;
- `rustok-page-builder` backend — capability policy, validation/sanitization,
  preview/review/materialization contracts, health and rollout controls;
- consumer backend — page/document revisions, immutable artifacts, publish
  transactions, receipts, outbox and cache ownership.

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
- [ ] Complete accessibility, nested-scroll, race and resource-limit evidence.
- [ ] Real-DOM storefront edit adapter.

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
- [x] Public Page Builder publication has no legacy/default-runtime lifecycle path;
  every builder document crosses the reviewed sanitizer/materialization pipeline.
- [ ] Complete HTML/CSS/URL/attribute policy and parser evidence is not integrated
  for the full reviewed publish surface.
- [ ] Observed tenant Wave 0/Wave 1 evidence is incomplete.

### Pages reference consumer

- [x] Pages admin mounts Page Builder through a module-owned facade.
- [x] Pages owns optimistic metadata versions, localized body revisions and
  transport selection.
- [x] Metadata-only patch and document-only save commands are separate.
- [x] The obsolete parallel JSON/CRUD UI and PageBlock persistence/fallback paths
  are deleted.
- [x] Pages provides one builder-first workspace with list/create/select,
  publish/unpublish and delete operations.
- [x] New/current documents use only `pages[].component`.
- [x] Pages storefront renders current Page Builder documents and static landing
  artifacts.
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
- [ ] Cache-consumer invalidation from the durable `NodePublished` outbox signal is
  not yet proven.
- [ ] Authenticated storefront inline editing is not implemented.

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
    -> published state + transactional outbox
    -> durable idempotent publish receipt
    -> cache/storefront correlation

  rustok-page-builder
    -> capability policy / health / rollout
    -> provider adapter seams
    -> preview/review/sanitization/materialization identity
```

Hosts are composition roots only. They supply route, locale, auth and tenant
context; they do not own Fly state, Pages policy or persistence.

## Dependency rules

```text
fly-ui -> fly
fly-leptos -> fly-ui + fly
rustok-page-builder-admin -> fly-leptos -> fly-ui -> fly
rustok-page-builder-storefront -> fly-leptos/fly-ui/fly as required
consumer admin/storefront -> public Page Builder contracts
consumer backend -> Fly/Page Builder validation and rendering contracts
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
- widget data does not flow through a generic builder facade;
- dynamic widgets store versioned configuration only;
- consumer list/create/lifecycle UI remains consumer-owned;
- no fallback editor is mounted when the provider is unavailable: the surface
  shows a typed degraded/read-only state.

## Security and operations

- Arbitrary component scripts are disabled.
- HTML, CSS, URLs and attributes require authoritative backend policy.
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
- Save, review, sanitization, publish receipt, artifact and storefront read share
  correlation identifiers.

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
- [ ] Real-DOM storefront overlay adapter.
- [ ] Accessibility, nested-scroll, race and resource-budget suites.

### Phase 4 — authoritative validation and sanitization

- [x] Add an explicit provider-owned static publish sanitization envelope and
  SHA-256 identity before runtime materialization.
- [x] Route the reviewed static publish path through current Fly traversal,
  structural validation, deterministic ids and secure public-resource checks.
- [x] Remove non-reviewed/default-runtime builder publication paths.
- [ ] Finalize complete HTML/CSS/URL/attribute policy and parser dependencies.
- [ ] Enforce all size/depth/assets/styles limits across the reviewed publish path.
- [ ] Add real-project runtime tests and accepted typed policy evidence.

### Phase 5 — consumer write separation

- [x] Add metadata-only patch commands.
- [x] Add document-only save commands with body revision.
- [x] Independently conflict-check metadata and document revisions.
- [ ] Move consumer metadata editing into typed property contributions.

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
- [x] Idempotent atomic Pages service: page/body locks, transactional feature and
  existing-baseline reads, sanitization, materialization, persist/bind, published
  state, `NodeUpdated`/`NodePublished` outbox and durable receipt.
- [x] Cut GraphQL, HTTP and admin transports over to the atomic reviewed service,
  reject create-and-publish and remove builder publication through the default
  runtime lifecycle.
- [x] Isolate non-builder publication behind explicitly named commands that reject
  every GrapesJS/Fly body before and inside the transaction.
- [ ] Prove route/page/artifact cache invalidation from the durable outbox signal.
- [ ] Rollback to previous immutable artifacts.
- [ ] Repair/rebuild and integrity-audit commands.

### Phase 7 — Page Builder admin

- [x] Manifest-backed FFA package and full-authoring shell.
- [x] Pages builder-first reference workspace.
- [x] Contribution assembly and capability policy foundations.
- [x] Complete reviewed-runtime scenario selection and deterministic idempotency
  transport UX at source level.
- [ ] Complete typed properties, assets, provider-health and degraded controls.
- [ ] Complete accessibility and bundle budgets.

### Phase 8 — storefront

- [x] Current published document/static artifact rendering foundations.
- [ ] Render only selected immutable published artifacts.
- [x] Verify Page Builder runtime materialization evidence before storefront read.
- [ ] Authenticated real-DOM editing and draft/published switching.
- [ ] Prove anonymous bundles exclude authoring code.
- [ ] Visual/accessibility parity across admin preview and published output.

### Phase 9 — generated contribution registries

- [ ] Separate admin/storefront factories.
- [ ] Generate from module metadata.
- [ ] Filter by tenant, permission, capability, policy and health.
- [ ] Duplicate, cycle, version and missing-provider diagnostics.

### Phase 10 — rollout

- [ ] Internal tenant Wave 0 with observed evidence.
- [ ] Pages Wave 1 after transport publication, cache and rollback gates.
- [ ] Media/Pages reusable sections.
- [ ] Blog, Forum, Product, Pricing, Taxonomy and SEO contributions.
- [ ] Additional modules only after renderer/property/cache ownership is proven.

## Immediate implementation order

1. Prove `NodePublished` outbox consumption invalidates every affected route,
   artifact and page cache key; correlate the receipt through storefront reads.
2. Add idempotent rollback to a previous immutable artifact set.
3. Complete Pages metadata property contributions and Page Builder asset/degraded
   controls.
4. Finish the reviewed HTML/CSS/URL/attribute policy and resource limits.
5. Implement authenticated real-DOM storefront editing and bundle exclusion.
6. Run accepted Rust/WASM/browser and observed tenant evidence.

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
node scripts/verify/verify-pages-ui-boundary.mjs
node --test scripts/verify/verify-pages-ui-boundary.test.mjs
node scripts/verify/verify-fly-admin-browser-runtime.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-scenario-baseline.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-transport-cutover.mjs
npm run verify:page-builder:fba:baseline
npm run verify:page-builder:consumer:pages
npm run verify:i18n:ui
npm run verify:i18n:contract
cargo deny check
cargo audit
```

Required evidence covers current GrapesJS/Fly round trips, iframe rejection and
cleanup, DnD/keyboard/accessibility, metadata/body revision conflicts,
authoritative sanitization, deterministic artifact and receipt integrity,
preview/static materialization parity, idempotent replay, publish/rollback/cache
correlation, anonymous bundle exclusion and provider degradation.

## Update rules

- This is the central cross-module Fly/Page Builder programme plan.
- Consumer local plans are updated in the same change.
- Checkboxes reflect merged source; gates require executed evidence.
- Contract changes require matching guardrails/tests.
- New dependencies require dependency records.
- Do not reintroduce shadow editors, component mirrors, consumer block fallbacks,
  raw runtime-context persistence or host-owned publication policy.
