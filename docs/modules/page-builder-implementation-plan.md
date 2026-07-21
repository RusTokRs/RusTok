---
id: doc://docs/modules/page-builder-implementation-plan.md
kind: development_plan
language: en
status: active
---

# Fly Ecosystem and Page Builder Implementation Plan

## Status legend

- `[x]` — implemented in the repository.
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
  preview/publish ports, health and rollout controls.

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
- [ ] Authoritative sanitization is not fully integrated into every publish path.
- [ ] Observed tenant Wave 0/Wave 1 evidence is incomplete.

### Pages reference consumer

- [x] Pages admin mounts Page Builder through a module-owned facade.
- [x] Pages owns optimistic body revisions and transport selection.
- [x] The obsolete parallel JSON/CRUD UI is deleted.
- [x] Pages provides one builder-first workspace with list/create/select,
  publish/unpublish and delete operations.
- [x] New/current documents use only `pages[].component`.
- [x] Admin `PageBlock` and block GraphQL fields are removed.
- [x] Pages storefront renders current Page Builder documents and static landing
  artifacts.
- [ ] Backend/storefront block persistence and fallback code still require full
  deletion.
- [ ] Typed metadata-only patch and document-only save commands are not separated.
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
    -> metadata/document revisions
    -> validation/sanitization port
    -> deterministic artifact build
    -> immutable artifact persistence
    -> atomic published pointer

  rustok-page-builder
    -> capability policy / health / rollout
    -> provider adapter seams
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
consumer backend -> Fly rendering/validation through explicit ports
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
- writes carry optimistic revisions;
- acknowledgement uses the exact dispatched document hash;
- widget data does not flow through a generic builder facade;
- dynamic widgets store versioned configuration only;
- consumer list/create/lifecycle UI remains consumer-owned;
- no fallback editor is mounted when the provider is unavailable: the surface
  shows a typed degraded/read-only state.

## Security and operations

- Arbitrary component scripts are disabled.
- HTML, CSS, URLs and attributes require authoritative backend policy.
- Storefront edit mode requires explicit authentication and authorization.
- Dynamic widgets cannot bypass module RBAC.
- Missing providers never cause silent deletion.
- Browser listeners/observers/subscriptions clean up deterministically.
- Project/history/observer/overlay limits are configurable.
- Anonymous storefront bundles exclude authoring assets.
- Artifact identity and integrity are verified before publication/read.
- Save, publish, artifact and storefront read share correlation identifiers.

## Implementation phases

### Phase 0 — current-only baseline

- [x] Define Fly layers and dependency rules.
- [x] Keep GrapesJS as behavioural compatibility reference.
- [x] Establish `pages[].component` as current authority.
- [x] Delete Pages parallel JSON/CRUD admin UI.
- [x] Remove frame copy/synchronization helpers from Pages.
- [x] Add guardrails rejecting deleted UI, frame sync and admin blocks.
- [ ] Delete the remaining backend/storefront `PageBlock` subsystem.

**Gate:** repository production source contains no obsolete page block or shadow
editor authority.

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

- [ ] Finalize HTML/CSS/URL/attribute policy and parser dependencies.
- [ ] Route backend document traversal through Fly.
- [ ] Enforce size/depth/assets/styles limits.
- [ ] Add real-project runtime tests and typed policy errors.

### Phase 5 — consumer write separation

- [ ] Add metadata-only patch commands.
- [ ] Add document-only save commands with body revision/hash.
- [ ] Independently conflict-check metadata and document revisions.
- [ ] Move consumer metadata editing into typed property contributions.

### Phase 6 — deterministic publication

- [x] Landing renderer and build identity.
- [x] Immutable Pages landing artifact entities/services.
- [ ] Atomic idempotent publish transaction and outbox/cache invalidation.
- [ ] Rollback to previous immutable artifacts.
- [ ] Repair/rebuild and integrity-audit commands.

### Phase 7 — Page Builder admin

- [x] Manifest-backed FFA package and full-authoring shell.
- [x] Pages builder-first reference workspace.
- [x] Contribution assembly and capability policy foundations.
- [ ] Complete typed properties, assets, provider-health and degraded controls.
- [ ] Complete accessibility and bundle budgets.

### Phase 8 — storefront

- [x] Current published document/static artifact rendering foundations.
- [ ] Render only selected immutable published artifacts.
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
- [ ] Pages Wave 1 after current-only, publication and rollback gates.
- [ ] Media/Pages reusable sections.
- [ ] Blog, Forum, Product, Pricing, Taxonomy and SEO contributions.
- [ ] Additional modules only after renderer/property/cache ownership is proven.

## Immediate implementation order

1. Delete backend/storefront `PageBlock`/`page_blocks`/`BlockService` production
   code and rewrite fresh-install migrations.
2. Separate Pages metadata patch from Fly document save.
3. Complete atomic artifact publication, rollback, correlation and repair.
4. Complete Page Builder property/asset/degraded-state controls.
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
npm run verify:page-builder:fba:baseline
npm run verify:page-builder:consumer:pages
npm run verify:i18n:ui
npm run verify:i18n:contract
cargo deny check
cargo audit
```

Required evidence covers current GrapesJS/Fly round trips, iframe rejection and
cleanup, DnD/keyboard/accessibility, revision/hash conflicts, deterministic
artifact integrity, publish/rollback correlation, anonymous bundle exclusion and
provider degradation.

## Update rules

- This is the central cross-module Fly/Page Builder programme plan.
- Consumer local plans are updated in the same change.
- Checkboxes reflect merged source; gates require executed evidence.
- Contract changes require matching guardrails/tests.
- New dependencies require dependency records.
- Do not reintroduce shadow editors, component mirrors, consumer block fallbacks
  or host-owned persistence.
