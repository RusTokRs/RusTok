---
id: doc://docs/modules/page-builder-implementation-plan.md
kind: development_plan
language: en
status: active
---

# Fly Ecosystem and Page Builder Implementation Plan

## Status legend

- `[x]` — verified in the current repository or completed by the phase.
- `[ ]` — not completed yet.
- A phase is complete only when every required task and its phase gate are checked.

## Decision summary

Fly is the planned Rust page-builder ecosystem. GrapesJS remains the behavioural and
project-format reference until Fly passes bidirectional compatibility gates.

The layer split is:

- `fly` — framework-neutral editor engine, project model, registries, commands, history and
  lossless GrapesJS compatibility;
- `fly-ui` — framework-neutral visual-editor state, policies and UI contracts;
- `fly-leptos` — the Leptos/browser implementation of `fly-ui`;
- `rustok-page-builder/admin` — optional full authoring and builder control UI;
- `rustok-page-builder/storefront` — optional in-context editing UI for public frontends;
- module-owned admin/storefront packages — owners of domain widgets, translations, renderers
  and their own FFA adapters;
- `rustok-page-builder` — backend FBA provider for validation, sanitization, preview, publish,
  rollout, permissions, persistence and rendering seams.

`fly-ui` is not a RusTok bridge, transport layer or module control surface. It describes how a
visual editor behaves independently from Leptos, Dioxus, routing, GraphQL, server functions and
RusTok deployment topology.

The Page Builder module has two classic UI packages because admin and storefront are separate
deployment surfaces. They share Fly libraries but retain separate routes, security, transport
facades, permissions, bundles and release profiles.

If Page Builder is absent, visual editing, palettes, drag-and-drop and builder management are
absent. Consumer modules keep CRUD, rendering and documented fallback paths. Canonical project
data is never deleted or rewritten because an editor surface is unavailable.

## Current verified baseline

- [x] The current Next editor mounts GrapesJS with `grapesjs-preset-webpage`.
- [x] It loads through `loadProjectData()` and saves through `getProjectData()`.
- [x] The stored contract is `grapesjs_v1`.
- [x] `rustok-page-builder` provides `preview`, `tree`, `properties` and `publish` capabilities.
- [x] The provider owns permission maps, typed errors, rollout profiles, health evidence and
  transport-neutral endpoint envelopes.
- [x] `rustok-pages` is the reference consumer.
- [x] Pages admin has a JSON project-data fallback.
- [x] `modules.toml` is the build-time source of truth for enabled modules.
- [x] The programme records separate `fly`, `fly-ui`, `fly-leptos`, admin and storefront roles.
- [x] Rich-text editing is an existing separate capability and is explicitly outside Fly scope.
- [ ] Real GrapesJS fixtures and a compatibility matrix are committed.
- [ ] `fly`, `fly-ui` and `fly-leptos` exist.
- [ ] Page Builder admin and storefront UI packages exist and are manifest-backed.
- [ ] Generated admin and storefront contribution registries exist.

## Target architecture

```text
                         EXTERNAL / REUSABLE FLY LAYERS

  fly
  engine, project model, registries, commands, history, codec
    ^
    |
  fly-ui
  framework-neutral visual-editor state, intents and policies
    ^
    |
  fly-leptos
  Leptos DOM, canvas, browser events, DnD, overlays and factories
    ^                                      ^
    |                                      |
  page-builder/admin                 page-builder/storefront
  full authoring + control UI        in-context editing + preview overlays
    ^                                      ^
    |                                      |
  module admin contributions         module storefront contributions

                              BACKEND

  rustok-page-builder
    +-- validation / sanitization / preview / publish
    +-- RBAC / tenant scope / rollout / health
    +-- persistence and rendering ports
    +-- may depend on `fly`
    +-- must not depend on UI crates
```

Hosts are technical composition roots only. They mount module-owned surfaces, provide runtime
context and include generated contribution factories. They do not own editor behaviour, widget
schemas, translations, transport selection or persistence semantics.

## Physical package layout

```text
crates/
  fly/                                  # standalone engine
  fly-ui/                               # standalone framework-neutral UI contracts
  fly-leptos/                           # standalone Leptos/browser implementation

  rustok-page-builder/
    src/                                # backend FBA provider

    admin/                              # optional admin deployment surface
      locales/
      src/
        core.rs
        model.rs
        transport/
          graphql_adapter.rs
          native_server_adapter.rs
        editor/
          full_editor.rs
          admin_canvas.rs
          admin_shell.rs
        control/
          registry.rs
          policies.rs
          presets.rs
          compatibility.rs
          health.rs
        ui/leptos.rs

    storefront/                         # optional storefront deployment surface
      locales/
      src/
        core.rs
        model.rs
        transport/
          graphql_adapter.rs
          native_server_adapter.rs
        editor/
          inline_editor.rs
          edit_overlay.rs
          draft_preview.rs
        ui/leptos.rs

  rustok-pages/
    admin/src/builder/
    storefront/src/builder/

  rustok-forum/
    admin/src/builder/
    storefront/src/builder/
```

There is no third shared RusTok UI package by default. Shared editor mechanics belong in
`fly-ui` and `fly-leptos`. A Page Builder support crate is introduced only after concrete
RusTok-specific duplication is demonstrated.

## Dependency rules

```text
fly-ui -> fly
fly-leptos -> fly-ui + fly

rustok-page-builder-admin -> fly-leptos -> fly-ui -> fly
rustok-page-builder-storefront -> fly-leptos -> fly-ui -> fly

module admin contribution -> Fly contribution contracts
module storefront contribution -> Fly contribution contracts

rustok-page-builder backend -> fly
```

Forbidden dependencies:

```text
fly -X-> leptos / dioxus / rustok-*
fly-ui -X-> leptos / dioxus / rustok-*
fly-leptos -X-> rustok-*

rustok-page-builder backend -X-> fly-ui / fly-leptos / admin / storefront
page-builder admin/storefront -X-> optional domain UI packages directly
module contribution -X-> host application code
```

Generated registries may depend on enabled contribution factories and pass them into Page
Builder surfaces. Page Builder packages must not hard-code every optional module.

## Reference implementation and compatibility contract

GrapesJS is the behavioural reference for the first stable Fly release.

| GrapesJS concept | Fly concept |
|---|---|
| Editor | `FlyEditor` |
| DomComponents | component registry and component tree |
| BlockManager | block registry |
| TraitManager | trait registry |
| Commands | command registry |
| StyleManager | style registry |
| SelectorManager | selector registry |
| AssetManager | asset registry and asset-provider ports |
| StorageManager | project codec plus consumer-owned persistence |
| Plugins | `FlyPlugin` |
| component views | framework renderers in `fly-leptos`, later other adapters |

The first canonical format remains `grapesjs_v1`. Fixtures must be captured from the real Next
GrapesJS editor through `getProjectData()`.

```text
GrapesJS getProjectData()
  -> Fly deserialize
  -> Fly inspect or mutate
  -> Fly serialize
  -> GrapesJS loadProjectData()
```

The round trip preserves pages, frames, hierarchy, attributes, styles, selectors, assets,
traits, plugin metadata, custom fields and unknown future fields. Unknown data remains lossless
even when Fly cannot interpret it.

## Fly ecosystem layers

### `fly`

`fly` owns:

- project and document model;
- lossless `grapesjs_v1` codec;
- component tree and mutations;
- editor state and selection;
- commands and undo/redo history;
- block, component, trait, style, selector and asset registries;
- plugin registration and dependency validation;
- validation reports;
- missing-provider and unknown-component preservation;
- generic built-in blocks.

Initial generic blocks include wrapper, section, container, rows, columns, grid, text, heading,
list, link, image, video, generic media, button, divider, spacer, basic form primitives and
restricted raw HTML where backend policy permits it.

`fly` does not contain RusTok domain widgets.

### `fly-ui`

`fly-ui` owns the reusable visual-editor model:

- full, inline, preview and read-only presentations;
- layout and panel state;
- palette, layers, traits, styles and asset-panel contracts;
- toolbar and viewport actions;
- editor intents and command-facing actions;
- selection and overlay models;
- framework-neutral drag-and-drop intents;
- property-editor and renderer contracts;
- contribution registry contracts;
- editor policies and capability state;
- generic message identifiers and accessibility metadata;
- clipboard/project-fragment contracts;
- revision and dirty-state contracts.

`fly-ui` contains no DOM, browser runtime, UI framework, RusTok transport, routing, tenant
loading, RBAC implementation or module-specific widgets.

### `fly-leptos`

`fly-leptos` implements `fly-ui` for Leptos and browsers:

- DOM and signals;
- canvas rendering;
- palette, layers and panels;
- pointer and keyboard interaction;
- drag-and-drop and resize handles;
- selection and inline overlays;
- Leptos renderer/property-editor factories;
- viewport/device controls;
- missing-provider placeholders;
- browser accessibility behaviour;
- canvas coordinate adapters and hit testing;
- iframe integration for isolated admin canvas;
- real-DOM overlay integration for storefront editing.

A clean Leptos application must build full and inline Fly editors without importing RusTok.

## Build-versus-adopt policy

Fly is a custom page builder, not a custom browser, HTML tokenizer or CSS tokenizer.

### Implement inside Fly

- project model and compatibility codec;
- component-tree mutation semantics;
- commands and history;
- block/component/trait/plugin registries;
- stable IDs and widget schema migrations;
- editor and contribution state machines;
- nesting and drop-zone rules;
- hit-test interpretation and insertion decisions;
- full/inline presentation semantics;
- missing-provider behaviour;
- clipboard fragment format and ID remapping;
- revision/conflict diagnostics;
- module contribution protocol.

### Use standard platform or low-level crates

- browser DOM/events through `wasm-bindgen`, `web-sys` and `js-sys`;
- serialization through `serde` and `serde_json`;
- errors through `thiserror`;
- property/invariant testing through `proptest`;
- Leptos for the first browser adapter.

These are infrastructure primitives, not page-builder frameworks.

### Candidate dependencies requiring a spike and dependency record

- `cssparser` for CSS tokenization and parsing;
- `html5ever` for standards-based HTML fragment parsing/serialization;
- `ammonia` or equivalent primitives for implementing backend sanitization policy;
- `indexmap`, `slotmap` or an equivalent collection only where stable ordering/handles justify
  the dependency.

A candidate is not adopted merely because it exists. The spike must record API fit, WASM and
native compatibility, maintenance, licence, transitive dependencies, security history, bundle
impact, feature flags and replacement strategy.

### Explicitly not adopted initially

- GrapesJS or another complete page-builder framework inside Fly;
- a JavaScript editor as the hidden source of truth;
- a general-purpose JS drag-and-drop framework as the component-tree authority;
- a CRDT/collaboration engine in the first implementation;
- a third-party rich-text editor as part of Fly;
- CSS transformation/minification tooling as the canonical project model.

## Rich-text boundary

Rich-text editing is a separate existing implementation and is not part of Fly.

Fly is responsible only for block-level placement, sizing, selection and lifecycle of a text or
rich-content component. Inline text marks, document selections, composition/IME handling,
formatting commands, paste normalization and rich-text schema belong to the dedicated rich-text
capability.

Integration happens through a stable component/property-editor seam:

```text
Fly component node
  -> rich-text component contract / opaque versioned payload
  -> existing rich-text editor UI when editing is activated
  -> updated payload returned through a Fly command
```

Fly must not duplicate or fork the rich-text model. The project codec preserves the rich-text
payload losslessly even when the editor capability is unavailable.

## Canvas and browser architecture

### Admin canvas isolation

The default admin authoring canvas uses an iframe adapter so that page CSS does not corrupt
editor chrome and editor CSS does not alter page output.

The adapter owns:

- loading storefront styles and theme context;
- mapping iframe and parent coordinates;
- viewport/device sizing;
- scroll and zoom synchronization;
- cross-document pointer/keyboard focus handling;
- safe message/event bridging;
- teardown and observer cleanup.

An iframe-free mode may exist for tests or controlled embedding, but is not the default full
editor architecture.

### Storefront canvas

Storefront inline editing operates over the real rendered DOM. It adds selection outlines,
insertion controls and toolbars without replacing the published renderer tree.

### Coordinate and hit-testing engine

`fly-leptos` translates browser geometry into framework-neutral Fly UI intents. It must account
for nested scrolling, iframe offsets, zoom, CSS transforms, sticky/fixed elements, resize,
auto-scroll, pointer capture and touch/pen input.

Hit testing produces candidate zones. `fly` and `fly-ui` decide whether a drop is legal and
whether it means before, inside or after. DOM order never becomes the project source of truth.

## Style and markup processing

### Style model

The canonical model distinguishes:

- inline component declarations;
- class and selector rules;
- pseudo-state rules;
- media queries;
- CSS variables and design tokens;
- browser-computed style.

Computed style is inspection data only and is never copied wholesale into canonical project
data.

CSS parsing may use a reviewed low-level parser, but style ownership, normalization rules and
GrapesJS compatibility remain Fly responsibilities.

### HTML import/export

HTML import is an optional adapter, not the canonical project codec.

```text
HTML fragment
  -> standards-based parser
  -> import policy
  -> Fly components
  -> unknown/raw fallback when conversion is unsafe
```

HTML must never be parsed with regular expressions. Exported HTML/CSS is derived output and does
not replace project data.

### Sanitization

Backend policy remains authoritative. Parser/sanitizer crates may supply primitives, but RusTok
owns the allowlists, URL policy, attributes, CSS restrictions, tenant policy and error mapping.
Editor preview is never considered sanitized publish output.

## Clipboard and project fragments

Fly defines a versioned fragment format for copy, cut, paste and duplication. A fragment records:

- selected component subtree;
- styles/selectors needed by that subtree;
- referenced assets;
- plugin/provider requirements;
- source project version;
- optional migration diagnostics.

Paste remaps component IDs, selector IDs and internal references. Missing providers are retained
as placeholders. Cross-project paste must not silently copy privileged resolved data from dynamic
widgets.

Plain HTML paste is routed through the optional HTML import adapter. Rich-text paste remains the
responsibility of the separate rich-text editor while it is active.

## Autosave, revisions and future collaboration

Fly does not persist documents, but exposes enough state for consumer-owned autosave:

- dirty revision;
- operation/command sequence number;
- last acknowledged revision;
- deterministic project hash where feasible;
- save-in-progress and save-failed state;
- revision-conflict diagnostics.

The first implementation uses optimistic revision checks. Real-time collaboration and CRDT are
deferred behind an adapter boundary and must not replace the canonical Fly project model without
a separate ADR and compatibility plan.

## Page Builder UI packages

### Admin

`rustok-page-builder/admin` owns full authoring, admin routes, Page Builder control screens,
registry diagnostics, block policies, presets, migration actions, provider health and its own
FFA facade. It mounts Fly in full presentation and does not persist consumer documents.

### Storefront

`rustok-page-builder/storefront` owns authenticated edit-mode activation, real-DOM overlays,
inline toolbar, insertion controls, draft/published switching and its own FFA facade. The same
crate may be deployed on multiple frontend servers with different endpoints, tenants, themes and
registries.

```text
admin.example.com -> page-builder-admin
site-a.example.com -> page-builder-storefront
site-b.example.com -> page-builder-storefront
site-c.example.com -> page-builder-storefront
```

These are four deployments but two Page Builder UI package implementations.

## Consumer ownership and FFA

Consumer modules own document lifecycle. Fly and Page Builder surfaces emit intents but do not
decide how a Page, Post, Forum layout or Product template is persisted.

Every deployable UI package has its own FFA boundary:

```text
page-builder admin -> admin facade -> native/server or GraphQL -> builder backend
page-builder storefront -> storefront facade -> GraphQL or native/server -> builder backend
module widget UI -> module facade -> native/server or GraphQL -> module backend
```

Rules:

- UI does not branch directly on transport;
- Fly crates never select RusTok transports;
- admin and storefront builder facades are separate;
- widget data never flows through the generic Page Builder facade;
- locale, tenant and auth come from host contracts;
- dynamic widgets store configuration, not resolved domain snapshots.

## Composition and enablement

Build profiles:

```text
no builder: no admin editor, no storefront edit mode
admin only: full authoring/control, no storefront edit mode
storefront only: inline editing, no admin control surface
full: admin + storefront
```

Admin and storefront have separate generated registries. Admin contributions provide blocks,
traits and editor renderers. Storefront contributions provide published renderers and optional
inline-edit integration. A module may support one surface without the other.

Runtime filtering applies tenant state, permissions, policies and capability health. Disabled
providers produce diagnostics/placeholders and never destructive conversion.

## Module UI support matrix

| Module UI | Planned role | Initial scope | Wave |
|---|---|---|---|
| `rustok-page-builder/admin` | full authoring and control | editor, policies, presets, compatibility, health | Admin |
| `rustok-page-builder/storefront` | inline authoring | in-context editing and draft preview | Storefront |
| `rustok-pages/admin` | consumer/block/trait provider | page editing, links, menus, reusable sections | Pilot |
| `rustok-pages/storefront` | renderer/optional inline editor | published layouts and Pages widgets | Pilot |
| `rustok-media/admin` | asset/block provider | media picker, images, gallery, video | A |
| `rustok-blog` UI packages | consumer, providers and renderers | posts, feeds, author card, templates | B |
| `rustok-forum` UI packages | consumer, providers and renderers | topics, categories, discussion layouts | B |
| `rustok-product` UI packages | providers and renderers | cards, grids, recommendations, templates | B |
| `rustok-pricing` UI packages | providers and renderers | price display and pricing tables | B |
| taxonomy owner UI | trait provider | selectors and query configuration | B |
| owner SEO panels | control contribution | metadata inspector | B |
| `rustok-commerce` UI packages | consumer/provider/renderer | merchandising composition | C |
| `rustok-search` UI packages | provider/renderer | search, results and facets | C |
| comments/profile owner UIs | future providers | after renderer and ownership contracts exist | C |

Order, Payment, Fulfillment, Inventory and Customer do not expose privileged operational actions
as page blocks by default.

## Plugin and widget versioning

Custom IDs are namespaced and stable, for example:

```text
rustok.forum.latest_topics
rustok.blog.featured_post
rustok.product.product_grid
```

Stored nodes carry provider and schema version. Provider modules own migrations. Missing
providers preserve complete nodes and unknown fields and allow deletion only by explicit user
action.

Fly, `fly-ui`, framework adapter and widget schema versions are separate. Generated HTML/CSS is
derived output and never replaces canonical project data.

## Dependency governance

Every new dependency requires a recorded decision covering:

- exact purpose and why local implementation is not appropriate;
- licence and repository policy compatibility;
- WASM and native support;
- maintenance and release activity;
- security history;
- transitive dependency count;
- feature flags and optionality;
- binary/WASM size impact;
- anonymous storefront bundle impact;
- replacement/exit strategy.

Required governance tooling should include or integrate equivalents of:

```text
cargo deny check
cargo audit
unused-dependency detection
licence allowlist
WASM bundle-size budgets
duplicate dependency reporting
```

Dependency features must be minimal. Storefront editing dependencies must not enter anonymous
bundles when edit mode is disabled.

## Security and operational requirements

- Arbitrary component scripts are disabled by default.
- Raw HTML, URLs, attributes and CSS pass backend sanitization.
- Storefront edit mode requires explicit authentication and authorization.
- Dynamic widgets cannot bypass module RBAC.
- Asset selection respects media permissions.
- Publish writes require deadlines and idempotency.
- Missing providers never cause silent deletion.
- Renderer failures become typed diagnostics.
- Dynamic widgets define cache keys and invalidation ownership.
- Admin, inline and published states have parity tests.
- Accessibility and keyboard interaction are acceptance requirements.
- Project size, history size, observer count and DOM overlay count have configurable limits.
- Browser observers, event listeners and iframe bridges are cleaned up deterministically.
- Anonymous storefront bundles exclude editor assets unless explicitly enabled.

## Implementation phases

### Phase 0 — Baseline, ADR and dependency policy

- [ ] **Phase status:** in progress.
- [x] Keep Next GrapesJS as the reference editor.
- [x] Keep `grapesjs_v1` as the current contract.
- [x] Preserve backend FBA boundaries and Pages JSON fallback.
- [x] Record the Fly layer and dual-surface split.
- [x] Record rich-text editing as an external existing capability.
- [ ] Add the architecture ADR.
- [ ] Add the build-versus-adopt and dependency-record templates.
- [ ] Capture real GrapesJS fixtures and plugin/version metadata.
- [ ] Add the Node `loadProjectData()` compatibility harness.
- [ ] Add dependency guards for Fly crates.

**Gate:** architecture, dependency policy and compatibility evidence are reproducible in CI.

### Phase 1 — `fly` engine and lossless codec

- [ ] Create `crates/fly` with docs.
- [ ] Implement lossless project model and typed accessors.
- [ ] Implement commands, history, registries, validation and missing-provider handling.
- [ ] Implement generic blocks and stable IDs.
- [ ] Define rich-text payload preservation/integration contracts without implementing rich text.
- [ ] Define versioned clipboard fragments and revision state.
- [ ] Add property-based and round-trip tests.

**Gate:** fixtures round-trip losslessly and command/history invariants pass.

### Phase 2 — Framework-neutral `fly-ui`

- [ ] Create `crates/fly-ui` with no framework or RusTok dependencies.
- [ ] Define presentation, panel, toolbar, selection and overlay state.
- [ ] Define DnD, hit-test result, clipboard and revision/conflict contracts.
- [ ] Define renderer/property-editor/contribution contracts.
- [ ] Define rich-text integration seam only.
- [ ] Add state-machine and policy tests.

**Gate:** a mock framework adapter drives full and inline editing solely through `fly-ui`.

### Phase 3 — Generic `fly-leptos`

- [ ] Create `crates/fly-leptos`.
- [ ] Implement DOM, canvas, panels, selection, DnD, resize and keyboard interaction.
- [ ] Implement coordinate transforms, hit testing and auto-scroll.
- [ ] Implement iframe admin adapter and real-DOM storefront overlay adapter.
- [ ] Implement browser observers and deterministic cleanup.
- [ ] Implement full, inline, preview and read-only modes.
- [ ] Add standalone examples, accessibility tests and browser interaction tests.

**Gate:** clean Leptos examples support full iframe editing and real-DOM inline editing.

### Phase 4 — Parser and sanitization spikes

- [ ] Evaluate CSS parser candidates.
- [ ] Evaluate HTML fragment parser candidates.
- [ ] Evaluate backend sanitizer primitives.
- [ ] Record dependency decisions, licences, WASM/native compatibility and bundle impact.
- [ ] Implement optional HTML import adapter only after approval.
- [ ] Keep project codec independent from HTML/CSS transformation tools.

**Gate:** approved primitives have dependency records and do not become canonical project models.

### Phase 5 — Backend Fly integration

- [ ] Depend on `fly` only.
- [ ] Route traversal and validation through Fly.
- [ ] Integrate preview/rendering through existing ports.
- [ ] Implement authoritative sanitization policy.
- [ ] Preserve capability envelopes and typed errors.
- [ ] Add runtime tests using GrapesJS projects.

**Gate:** Next GrapesJS projects validate, preview and publish through Fly without contract break.

### Phase 6 — Page Builder admin surface

- [ ] Create the admin UI package with standard FFA structure.
- [ ] Implement full authoring shell over the iframe Fly adapter.
- [ ] Integrate admin theme, locale, permissions and degraded state.
- [ ] Implement admin transports and control screens.
- [ ] Add dependency and WASM-size budgets.

**Gate:** admin authoring works in embedded and headless profiles and disappears when uncomposed.

### Phase 7 — Page Builder storefront surface

- [ ] Create the storefront UI package with standard FFA structure.
- [ ] Implement authenticated edit mode over real storefront DOM.
- [ ] Implement overlays, insertion controls, draft preview and edit exit.
- [ ] Implement storefront transports and multi-deployment configuration.
- [ ] Ensure anonymous bundles exclude editor code when disabled.

**Gate:** one storefront crate supports multiple servers and optional edit mode without bundle leak.

### Phase 8 — Generated contribution registries

- [ ] Define separate admin and storefront factories.
- [ ] Extend module manifests with surface metadata.
- [ ] Generate registries without hard-coded optional dependencies.
- [ ] Apply tenant, permission and policy filtering.
- [ ] Add duplicate, cycle and missing-provider diagnostics.

**Gate:** each surface composes only available contributions without deleting unavailable nodes.

### Phase 9 — Pages pilot across both surfaces

- [ ] Add Pages admin blocks, traits, renderers and translations.
- [ ] Add Pages storefront renderers and optional inline editing.
- [ ] Keep lifecycle and transports in Pages FFA packages.
- [ ] Preserve JSON fallback.
- [ ] Integrate the existing rich-text capability through the defined seam where needed.
- [ ] Test no-builder, admin-only, storefront-only and full profiles.
- [ ] Add GrapesJS/Fly cross-editor round trips.

**Gate:** one Page is editable through GrapesJS, Fly admin and Fly storefront without data loss.

### Phase 10 — Module rollout

- [ ] Wave A: Media and Pages reusable sections.
- [ ] Wave B: Blog, Forum, Product, Pricing, Taxonomy and SEO contributions.
- [ ] Wave C: Commerce, Search, Comments and Profiles where contracts exist.
- [ ] Require per-surface translations, renderer ownership, FFA parity and cache policy.
- [ ] Require behaviour documentation for missing Page Builder surfaces/providers.

**Gate:** modules enable independently on admin/storefront without destructive project changes.

### Phase 11 — Published rendering and rollout completion

- [ ] Stabilize widget configuration contracts.
- [ ] Complete storefront renderers.
- [ ] Define safe derived HTML/CSS caching.
- [ ] Define cache invalidation ownership.
- [ ] Correlate save, publish, lifecycle and storefront read.
- [ ] Complete rollback and legacy bridge exit.

**Gate:** admin preview, inline editing and published output have verified parity.

### Phase 12 — Future adapters and collaboration

- [ ] Add `fly-dioxus` only after `fly-ui` stabilizes.
- [ ] Keep rich-text integration framework-independent and external.
- [ ] Evaluate collaboration only after optimistic revision handling is proven.
- [ ] Require a separate ADR before adopting CRDT or changing canonical project semantics.

### Phase 13 — Optional Fly repository extraction

- [ ] Confirm no RusTok dependency leakage.
- [ ] Stabilize APIs and semantic versioning.
- [ ] Decide licence and publication policy.
- [ ] Extract Fly crates only when independent release cadence is valuable.
- [ ] Keep Page Builder admin/storefront in RusTok.

## Verification programme

```text
cargo test -p fly
cargo test -p fly-ui
cargo test -p fly-leptos
cargo test -p rustok-page-builder
cargo test -p rustok-page-builder-admin
cargo test -p rustok-page-builder-storefront
cargo test -p rustok-pages-admin
cargo test -p rustok-pages-storefront
cargo xtask module validate page_builder
cargo xtask module validate pages
npm run verify:page-builder:fba:baseline
npm run verify:i18n:ui
npm run verify:i18n:contract
cargo deny check
cargo audit
```

Required suites cover:

- GrapesJS round trips and Node reload;
- unknown fields/providers and migrations;
- commands, history, clipboard fragments and revision conflicts;
- `fly-ui` state machines;
- iframe coordinate mapping and real-DOM overlays;
- DnD hit testing, zoom, scrolling and auto-scroll;
- browser observer/listener cleanup;
- admin/storefront profiles and multiple storefront deployments;
- transport parity per surface;
- renderer parity and missing-provider fallback;
- rich-text payload preservation and integration seam, not rich-text behaviour;
- sanitization and script rejection;
- dependency licences, advisories and bundle budgets;
- accessibility, keyboard operation and resource limits;
- publish idempotency, rollback and cache invalidation.

## Update rules

- This document is the central cross-module Fly programme plan.
- The local Page Builder backend plan links here when implementation starts.
- Contributing modules update their own plans before rollout tasks are completed here.
- Contract changes require matching verification changes.
- Checkboxes are updated only from merged code and reproducible evidence.
- New dependencies require dependency records in the same change.
- Remove wording that treats `fly-ui` as a RusTok bridge, merges admin/storefront into one
  deployment package, places editor ownership in hosts or describes rich-text editing as Fly
  responsibility.
