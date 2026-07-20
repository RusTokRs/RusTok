---
id: doc://docs/modules/page-builder-implementation-plan.md
kind: development_plan
language: en
status: active
---

# Fly Ecosystem and Page Builder Implementation Plan

## Status legend

- `[x]` — implemented in the current repository.
- `[ ]` — not implemented or not reproducibly verified yet.
- A phase gate stays open until its required Rust, WASM, browser and compatibility evidence has
  actually been run, even when the corresponding source code exists.

## Decision summary

Fly is the custom Rust page-builder ecosystem. GrapesJS remains the behavioural and
`grapesjs` compatibility reference until bidirectional browser round trips are proven.

The stable layer split is:

- `fly` — framework-neutral project model, codec, editor commands, history, registries,
  validation, fragments and revision state;
- `fly-ui` — framework-neutral presentation, selection, overlays, DnD intents, policies and
  contribution contracts;
- `fly-leptos` — Leptos/browser integration, coordinate adapters, lifecycle handles and iframe
  bridge primitives;
- `rustok-page-builder/admin` — optional full-authoring shell and canonical admin FFA facade;
- `rustok-page-builder/storefront` — future authenticated real-DOM inline-editing surface;
- consumer module UI packages — owners of document lifecycle, metadata, persistence adapters and
  domain contributions;
- `rustok-page-builder` — backend FBA provider for capability policy, validation, sanitization,
  preview, publish, health and rollout seams.

`fly-ui` is not a RusTok transport or bridge. Fly packages do not choose GraphQL, server
functions, tenant policy or domain persistence. Rich-text editing remains a separate existing
capability and is explicitly outside Fly scope.

## Current verified repository baseline

- [x] Next GrapesJS remains the behavioural reference and uses `loadProjectData()` /
  `getProjectData()`.
- [x] `grapesjs` remains the canonical stored contract.
- [x] `fly`, `fly-ui` and `fly-leptos` exist as separate crates.
- [x] The Fly project model preserves unknown fields/providers and exposes stable IDs, commands,
  history, registries, validation, clipboard fragments and revision hashes.
- [x] `rustok-page-builder/admin` exists, is manifest-backed and is composed by `apps/admin` for
  CSR, hydration and SSR profiles.
- [x] The admin package has an isolated iframe renderer, source/origin/protocol/instance/sequence
  validation, viewport/geometry messages, hover/selection overlays and facade-owned save
  lifecycle handling.
- [x] `rustok-pages/admin` is the reference consumer and mounts Page Builder through
  `PageBuilderAdminHostContext`.
- [x] Pages performs optimistic `updated_at` conflict checks and persists through its existing
  module-owned transport facade.
- [x] Pages treats `pages[].component` as canonical, synchronizes legacy mirrored frame
  roots, and preserves real GrapesJS frame scaffold metadata losslessly.
- [x] Source-level dependency, browser-boundary and Pages-consumer guards exist.
- [x] Real GrapesJS browser captures and a compatibility matrix are committed.
- [ ] Rust/WASM/browser suites have been run for the current integration slice.
- [ ] Page Builder storefront UI and real-DOM inline editing exist.
- [ ] Generated admin/storefront contribution registries exist.
- [ ] Full palette, drag/drop, resize, keyboard, traits, styles and asset authoring exist.

## Implemented slice — 2026-07-13

The current admin pilot supports this end-to-end path:

```text
selected Pages document
  -> Pages loads current grapesjs project
  -> Pages canonicalizes pages[].component
  -> PageBuilderAdminHostContext
  -> AdminCanvasController / FlyEditor
  -> isolated srcdoc iframe renderer
  -> geometry, viewport, hover, focus and pointer messages
  -> FlyUiStateMachine selection and overlays
  -> canonical PageBuilderCapabilityRequest::Publish
  -> PagesBuilderFacade
  -> latest Page read + optimistic revision check
  -> existing Pages transport::update_page
  -> backend updated_at acknowledgement against dispatched Fly hash
```

The iframe renderer:

- uses CSP and an allowlisted HTML projection;
- rejects event attributes, `javascript:` URLs and unsafe inline style constructs;
- instruments components with stable Fly IDs;
- emits geometry through `ResizeObserver` and `getBoundingClientRect()`;
- repeats its handshake after load to avoid listener-installation races;
- runs in `sandbox="allow-scripts"` without same-origin privileges;
- is accepted only when the parent sees the exact iframe `contentWindow`, opaque `null` origin,
  correct protocol/instance and a monotonically increasing sequence.

This renderer is an editor preview projection, not authoritative backend-sanitized publish output.

## Target architecture

```text
                         REUSABLE FLY LAYERS

  fly
  project model, codec, registries, commands, history, validation
    ^
    |
  fly-ui
  framework-neutral editor state, policies, DnD and contributions
    ^
    |
  fly-leptos
  Leptos/browser lifecycle, coordinates and adapter primitives
    ^                                      ^
    |                                      |
  page-builder/admin                 page-builder/storefront
  isolated full authoring            future real-DOM inline editing
    ^                                      ^
    |                                      |
  consumer admin facade              consumer storefront facade

                              BACKEND

  rustok-page-builder
    +-- capability policy / health / rollout
    +-- validation / sanitization / preview / publish seams
    +-- persistence and rendering ports
```

Hosts are technical composition roots only. They mount module-owned packages and provide route,
locale, auth and tenant context. They do not own Fly state, transport selection, widget schemas or
consumer persistence semantics.

## Dependency rules

```text
fly-ui -> fly
fly-leptos -> fly-ui + fly
rustok-page-builder-admin -> fly-leptos -> fly-ui -> fly
rustok-page-builder-storefront -> fly-leptos -> fly-ui -> fly
consumer admin/storefront package -> Page Builder facade contracts
rustok-page-builder backend -> fly only when backend traversal is integrated
```

Forbidden dependencies:

```text
fly -X-> leptos / dioxus / rustok-*
fly-ui -X-> leptos / dioxus / rustok-*
fly-leptos -X-> rustok-*
rustok-page-builder backend -X-> fly-ui / fly-leptos / admin / storefront
page-builder admin/storefront -X-> optional domain UI packages directly
consumer builder facade -X-> host application code
```

Every shared facade stored in Leptos owner context must be `Send + Sync`; browser futures may stay
local and execute through `spawn_local`.

## Compatibility contract

GrapesJS remains the reference:

```text
GrapesJS getProjectData()
  -> Fly deserialize
  -> Fly inspect or mutate
  -> Fly serialize
  -> GrapesJS loadProjectData()
```

Fly codec round trips preserve pages, frames, hierarchy, attributes, styles, selectors,
assets, traits, plugin metadata, custom fields and unknown future fields. Browser round trips are
evaluated separately against fields GrapesJS itself retains. The manifest contains a real
current-runtime capture and records structural normalization explicitly; real captures cannot
declare normalization exceptions.

For the Pages pilot:

- `pages[].component` is the canonical Fly component root;
- legacy mirrored `frames[0].component` trees remain synchronized;
- real GrapesJS frame scaffold objects remain opaque and are not overwritten by the canonical tree;
- canonical data wins for editor traversal while frame metadata and existing Page blocks are preserved;
- the compatibility mirror must be removed only through an explicit migration plan.

## Fly ecosystem responsibilities

### `fly`

`fly` owns project/document data, the lossless codec, component-tree mutations, selection,
commands, undo/redo, registries, validation, stable IDs, unknown-provider preservation,
versioned fragments and deterministic revision hashes. It does not contain RusTok domain widgets.

### `fly-ui`

`fly-ui` owns presentation modes, panels, viewport state, selection, overlays, DnD intents,
hit-test candidates, renderer/property-editor contracts, contribution registries, capabilities,
clipboard state and dirty/save policy. It contains no DOM, Leptos, routing or RusTok transport.

### `fly-leptos`

`fly-leptos` owns browser lifecycle and framework adaptation. The current implementation includes
coordinate structures, iframe message validation, RAII event listeners, resize observers, pointer
capture and an extensible iframe JSON subscription. Full DnD, resize, keyboard and real-DOM
storefront adapters remain open.

## Rich-text boundary

Fly owns block-level placement and lifecycle of a rich-content component. Inline marks, document
selection, IME/composition handling, formatting, paste normalization and the rich-text schema stay
in the dedicated rich-text capability. Fly preserves its opaque versioned payload and invokes it
through renderer/property-editor seams; it does not fork or replace the rich-text model.

## Consumer ownership and FFA

Consumer modules own document lifecycle:

```text
Page Builder admin
  -> canonical PageBuilderAdminFacade
  -> consumer facade
  -> consumer transport facade
  -> consumer backend
```

Rules:

- UI does not branch directly on transport;
- Fly packages never select RusTok transports;
- consumer metadata is refreshed before save rather than captured once in a stale editor closure;
- writes carry an optimistic consumer revision;
- Fly acknowledgement uses the exact project hash dispatched by the save request;
- widget data never flows through the generic Page Builder facade;
- dynamic widgets store configuration, not resolved privileged snapshots.

## Build-versus-adopt policy

Fly is a custom builder, not a custom browser, HTML tokenizer or CSS tokenizer. Standard low-level
browser and serialization crates are allowed. Complete page-builder frameworks, hidden JavaScript
sources of truth, generic JS DnD authorities, CRDT engines and rich-text editors are not adopted as
Fly internals.

Parser or sanitizer candidates such as `cssparser`, `html5ever` or `ammonia` require an explicit
spike and dependency record covering licence, maintenance, security, WASM/native compatibility,
transitive dependencies, bundle impact and exit strategy. Backend sanitization policy remains
RusTok-owned and authoritative.

## Security and operational requirements

- Arbitrary component scripts are disabled by default.
- Raw HTML, URLs, attributes and CSS require authoritative backend sanitization.
- Storefront edit mode requires explicit authentication and authorization.
- Dynamic widgets cannot bypass module RBAC.
- Missing providers never cause silent deletion.
- Browser listeners, observers and iframe subscriptions clean up deterministically.
- Project/history/observer/overlay limits remain configurable.
- Anonymous storefront bundles must exclude editor assets unless edit mode is explicitly enabled.
- Admin, inline and published states require parity and accessibility evidence.

## Implementation phases

### Phase 0 — Baseline, ADR and dependency policy

- [x] Keep Next GrapesJS and `grapesjs` as the reference.
- [x] Preserve backend FBA boundaries and Pages JSON fallback.
- [x] Record the Fly layers, dual deployment surfaces and rich-text exclusion.
- [x] Add the Fly architecture ADR.
- [x] Add dependency-record and build-versus-adopt templates.
- [x] Add source-level dependency and compatibility guards.
- [x] Add the Node compatibility harness structure.
- [ ] Capture real GrapesJS browser fixtures and exact plugin/version metadata.

**Gate:** open until real fixtures and compatibility evidence run reproducibly.

### Phase 1 — `fly` engine and lossless codec

- [x] Create `crates/fly`.
- [x] Implement project model, typed accessors and unknown-field preservation.
- [x] Implement commands, history, registries, validation and missing-provider handling.
- [x] Implement stable IDs and generic built-in registry entries.
- [x] Define the external rich-text payload seam.
- [x] Define versioned fragments and revision state.
- [x] Add unit/property/round-trip test sources.
- [ ] Verify current tests against real GrapesJS browser fixtures.

**Gate:** open until the Rust suite and real fixture round trips pass.

### Phase 2 — Framework-neutral `fly-ui`

- [x] Create `crates/fly-ui` without framework or RusTok dependencies.
- [x] Define presentation, panels, viewport, selection and overlays.
- [x] Define DnD/hit-test, clipboard and revision/conflict contracts.
- [x] Define renderer/property-editor/contribution contracts.
- [x] Define only the rich-text integration seam.
- [x] Add state-machine and policy test sources.
- [ ] Prove a mock adapter drives complete full and inline editing.

**Gate:** open pending executable adapter tests.

### Phase 3 — Generic `fly-leptos`

- [x] Create `crates/fly-leptos`.
- [x] Add coordinate transforms and framework-neutral browser geometry structures.
- [x] Add RAII event listeners, resize observers and pointer capture.
- [x] Add source/origin validated iframe subscriptions and teardown ownership.
- [x] Connect iframe viewport, geometry, hover and selection to Fly UI overlays.
- [x] Implement palette, DnD, resize, auto-scroll and keyboard interaction.
- [ ] Implement the real-DOM storefront overlay adapter.
- [ ] Add standalone examples, accessibility and browser interaction suites.

**Gate:** open pending complete iframe editing and real-DOM inline examples.

### Phase 4 — Parser and sanitization spikes

- [ ] Evaluate CSS parser candidates.
- [ ] Evaluate HTML fragment parser candidates.
- [ ] Evaluate backend sanitizer primitives.
- [ ] Record dependency decisions and bundle impact.
- [ ] Implement optional HTML import only after approval.

**Gate:** open.

### Phase 5 — Backend Fly integration

- [ ] Route backend traversal and validation through `fly`.
- [ ] Integrate preview/rendering through existing ports.
- [ ] Implement authoritative sanitization policy.
- [x] Preserve capability envelopes and typed errors.
- [ ] Add runtime tests using real GrapesJS projects.

**Gate:** open.

### Phase 6 — Page Builder admin surface

- [x] Create the manifest-backed admin UI package with FFA structure.
- [x] Add full-presentation shell and isolated iframe projection.
- [x] Integrate host locale and CSR/hydrate/SSR composition.
- [x] Add canonical context-safe facade and save lifecycle.
- [x] Add initial diagnostics, undo/redo, geometry, hover, selection and overlays.
- [ ] Integrate permission/degraded-state policy into editor controls.
- [ ] Implement control screens, presets and provider health UI.
- [ ] Add dependency and WASM-size budgets.
- [ ] Complete palette/DnD/property/asset authoring.

**Gate:** open pending executable embedded/headless and full-authoring evidence.

### Phase 7 — Page Builder storefront surface

- [ ] Create the storefront UI package.
- [ ] Implement authenticated real-DOM edit mode, overlays and insertion controls.
- [ ] Implement draft/published switching and storefront facade.
- [ ] Prove anonymous bundles exclude editor code.

**Gate:** open.

### Phase 8 — Generated contribution registries

- [ ] Define separate admin and storefront factories.
- [ ] Generate registries from module metadata.
- [ ] Apply tenant, permission, policy and health filtering.
- [ ] Add duplicate, cycle and missing-provider diagnostics.

**Gate:** open.

### Phase 9 — Pages pilot

- [x] Mount Page Builder through the Pages module-owned admin package.
- [x] Keep page lifecycle and transport selection in Pages FFA code.
- [x] Preserve the old JSON editor and existing blocks.
- [x] Add canonical/legacy component migration and synchronization.
- [x] Add optimistic backend revision conflict handling.
- [x] Return the backend revision to Fly and acknowledge the dispatched hash.
- [x] Add English/Russian messages and source-level consumer guards.
- [ ] Add Pages block/trait/renderer contribution factories.
- [ ] Add Pages storefront renderers and optional inline editing.
- [ ] Integrate the existing rich-text capability where a text editor is activated.
- [ ] Test no-builder, admin-only, storefront-only and full profiles.
- [x] Add real GrapesJS/Fly cross-editor round trips.
- [x] Add production-browser contracts for iframe handshake, source/origin/instance rejection,
  sequence replay protection, geometry/zoom overlays, teardown cleanup and fail-closed stale-save
  revision handling with an explicit refreshed retry.

**Gate:** open pending complete Rust/WASM execution and both-surface data-loss tests.

### Phase 10 — Module rollout

- [ ] Wave A: Media and Pages reusable sections.
- [ ] Wave B: Blog, Forum, Product, Pricing, Taxonomy and SEO contributions.
- [ ] Wave C: Commerce, Search, Comments and Profiles where contracts exist.
- [ ] Require per-surface translations, renderer ownership, FFA parity and cache policy.

### Phase 11 — Published rendering and rollout completion

- [ ] Stabilize widget configuration contracts and storefront renderers.
- [ ] Define safe derived HTML/CSS caching and invalidation ownership.
- [ ] Correlate save, publish, lifecycle and storefront read.
- [ ] Complete rollback and legacy bridge exit.

### Phase 12 — Future adapters and collaboration

- [ ] Add `fly-dioxus` only after `fly-ui` stabilizes.
- [x] Keep rich-text integration framework-independent and external.
- [ ] Evaluate collaboration only after optimistic revision handling is proven.
- [ ] Require a separate ADR before CRDT adoption or canonical model changes.

### Phase 13 — Optional Fly repository extraction

- [ ] Confirm no RusTok dependency leakage.
- [ ] Stabilize public APIs and semantic versioning.
- [ ] Decide licence/publication policy.
- [ ] Extract only when an independent release cadence is valuable.
- [x] Keep Page Builder admin/storefront packages in RusTok.

## Immediate next implementation order

1. Run the full Rust, WASM and browser suites against the current real GrapesJS capture and
   retain reproducible evidence.
2. Extend the established browser contract suite with nested scrolling, DnD race coverage,
   accessibility and resource limits.
3. Complete capability/degraded-state policy, asset authoring and accessibility coverage.
4. Integrate authoritative backend Fly traversal/sanitization before treating iframe output as
   publish-ready rendering.
5. Implement the separate storefront real-DOM editing package.
6. Add `fly-dioxus` only after the public `fly-ui` adapter contract stabilizes.

## Verification programme

```text
cargo test -p fly
cargo test -p fly-ui
cargo test -p fly-leptos
cargo test -p rustok-page-builder
cargo test -p rustok-page-builder-admin
cargo test -p rustok-pages-admin
cargo xtask module validate page_builder
cargo xtask module validate pages
node scripts/verify/verify-fly-admin-browser-runtime.mjs
node scripts/verify/verify-pages-ui-boundary.mjs
npm run verify:page-builder:fba:baseline
npm run verify:page-builder:consumer:pages
npm run verify:page-builder:pages:legacy-bridge
npm run verify:i18n:ui
npm run verify:i18n:contract
cargo deny check
cargo audit
```

Required browser suites cover iframe handshake and rejection paths, component geometry, nested
scrolling, selection/hover overlays, DnD hit testing, cleanup, save races, stale consumer revisions,
accessibility and resource limits.

## Update rules

- This document is the central cross-module Fly programme plan.
- Checkboxes reflect merged source only; phase gates require reproducible executed evidence.
- Contributing modules update their local plans in the same change.
- Contract changes require matching verification changes.
- New dependencies require dependency records.
- Do not move transport, persistence, rich-text behavior or module widget ownership into Fly.
