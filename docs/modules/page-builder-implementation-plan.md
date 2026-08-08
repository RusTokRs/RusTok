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

This central plan is reconciled to current `main` as of 2026-08-08. The detailed
source overlays under `docs/modules/page-builder-*-actualization-2026-08-07.md`
and the 2026-08-08 Pages/Page Builder parity packets remain authoritative where
they are more specific. Older open checkboxes for Pages metadata contributions,
immutable rollback, artifact audit/repair, reviewed static resource limits,
authenticated real-DOM authoring, anonymous authoring exclusion, generated
contribution-registry foundations and shared contribution metadata tooling were
stale and are corrected below.

Current source markers for this slice:

```text
Provider status/degraded controls: source-ready
Pages module-metadata contribution generation: source-ready
Shared module contribution tooling: source-ready
Forum second-consumer contribution discovery: source-ready
Forum Fly adapter/component registry: open
```

Observed provider-health evidence remains an execution/composition cursor; an
absent live health snapshot is represented as `unobserved`, not healthy.

The Pages reference consumer keeps its complete Fly contribution declaration in
canonical `rustok-module.toml`. `rustok-pages-admin/build.rs` now delegates generic
parsing, provider/version injection and capability validation to the reusable
`rustok-build/src/module_manifest_contribution.rs` tooling source, while retaining
only Pages-specific role/constant assertions. `xtask module validate` consumes the
same normalizer for publish readiness. Admin/WASM runtime still does not parse TOML
and no handwritten Pages `ContributionDescriptor` tree remains.

Forum is now the second production consumer selected for the shared metadata
boundary. Its canonical `rustok-module.toml` declares a versioned owner-provider
`rustok.forum.widget-catalog` discovery contribution guarded by `forum_topics:read`
and `preview`, while preserving Forum as widget persistence/authorization owner.
The entry deliberately has no Fly blocks, renderers, property editors or storefront
surface: Forum does not yet have a real Fly component registry/adapter, so the
metadata records that adapter state as pending instead of fabricating runtime
capability. The next source cursor is that real Forum adapter/component-registry
slice. Live SLO health remains a separate open cursor.

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
- `rustok-page-builder/admin` — full-authoring shell, canonical admin FFA facade
  and typed provider status/degraded controls;
- `rustok-page-builder/storefront` — published rendering and authenticated
  real-DOM editing support;
- consumer admin/storefront packages — document lifecycle, metadata, transport,
  persistence adapters and domain contributions;
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
- [ ] Accepted parser/real-project/runtime policy evidence is incomplete.
- [ ] Observed provider-health and tenant Wave 0/Wave 1 evidence is incomplete.

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
  server handler composition. Live SLO health is deliberately `unobserved` until a
  real source exists.
- [ ] Accepted evidence must correlate outbox delivery, repair/rollback receipts,
  generation rotation, cache miss/refill, browser authoring and provider status.

### Forum second contribution consumer

- [x] Forum already owns versioned `topic_list`, `topic_detail` and `reply_stream`
  widget contracts plus catalog/validation HTTP routes guarded by
  `forum_topics:read`.
- [x] Canonical `rustok-module.toml` now declares the Forum owner-provider
  contribution discovery entry through the shared module metadata boundary.
- [x] The topic-detail manifest schema id is distinct (`forum.topic_detail.v1`)
  instead of incorrectly reusing the topic-list schema id.
- [x] Contribution discovery remains fail-closed with `blocks = []`, no renderers,
  no property editors and no storefront claim while the adapter is pending.
- [ ] Define actual Forum Fly component/block identities and a real adapter before
  exposing authoring or rendering contributions.
- [ ] Retain Forum Page Builder runtime/browser and observed Wave evidence.

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

  consumer domain (Forum)
    -> topic/reply lifecycle, revisions and visibility remain Forum-owned
    -> versioned widget catalog and props validation remain Forum-owned
    -> canonical contribution discovery metadata is shared-tooling validated
    -> Fly component/block/adapter runtime remains pending

  rustok-page-builder
    -> capability policy / health / rollout
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
- provider status may only narrow host tenant/RBAC capabilities;
- missing provider-health observation remains `unobserved` rather than healthy;
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
- Provider rollout flags and observed health are separate evidence. Lack of an
  observed SLO snapshot is never converted to a healthy claim.
- Consumer contribution metadata is canonical module metadata; runtime source must
  not retain a parallel handwritten descriptor tree.
- Shared contribution parsing/normalization is platform build tooling and publish
  validation only; it must not become runtime registry, tenant or persistence policy.
- A discovery contribution with no real adapter must not claim blocks, renderers,
  property editors or storefront runtime surfaces.

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
- [ ] Complete remaining generic typed asset/control surfaces and accessibility
  evidence.
- [ ] Retain observed provider-health/degraded browser evidence.

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
  discovery metadata without a consumer-local parser or schema authority.
- [ ] Connect Forum to runtime Fly component/block/adapter contribution assembly.

### Phase 10 — rollout

- [ ] Internal tenant Wave 0 with observed evidence.
- [ ] Pages Wave 1 after accepted publication/cache/rollback/repair/browser gates.
- [x] Forum canonical contribution discovery metadata through shared tooling.
- [ ] Forum Fly adapter/component registry and runtime contribution assembly.
- [ ] Media/Pages reusable sections.
- [ ] Blog, Product, Pricing, Taxonomy and SEO contributions.
- [ ] Additional modules only after renderer/property/cache ownership is proven.

## Immediate implementation order

1. Define the real Forum Fly component/block identities and adapter behavior against
   the existing Forum-owned widget catalog; only then make Forum contribution
   blocks/renderers/property editors non-empty and mount runtime assembly.
2. Connect a real provider-health observation source to the admin status seam and
   retain observed provider-health evidence for degraded/unavailable behavior.
3. Retain accepted Pages execution evidence for reviewed publish, rollback/repair,
   cache rotation/miss-refill, metadata isolation and authenticated/anonymous
   authoring boundaries.
4. Complete remaining generic Page Builder asset/accessibility controls and their
   executable browser evidence.
5. Promote no new consumer or FFA/FBA wave until its ownership and retained
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
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs
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
refill, anonymous authoring exclusion, provider degradation, generated module
metadata authority, shared publish validation, Forum discovery-to-real-adapter
continuity and observed tenant rollout.

## Update rules

- This is the central cross-module Fly/Page Builder programme plan.
- Consumer local plans are updated in the same change.
- Checkboxes reflect merged source; gates require executed evidence.
- Contract changes require matching guardrails/tests.
- New dependencies require dependency records.
- Do not reintroduce shadow editors, component mirrors, consumer block fallbacks,
  raw runtime-context persistence or host-owned publication/cache policy.
