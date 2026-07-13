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

The architecture is split into distinct ownership zones:

- `fly` is a standalone, framework-neutral Rust editor engine and GrapesJS-compatible
  project runtime;
- `fly-leptos` is a generic Leptos adapter that can be embedded in any Leptos frontend;
- `fly-ui` is the RusTok UI bridge over Fly and `fly-leptos`;
- module-owned UI packages contribute their own blocks, widgets, traits, settings,
  translations, editor renderers and published renderers;
- every module-owned UI keeps its own FFA `core/transport/ui` boundary and selects the
  native monolith or GraphQL/headless transport through its own frontend adapter;
- `rustok-page-builder` remains the backend FBA provider for validation, sanitization,
  preview, publish, rollout, permissions, persistence and rendering seams;
- the visual editor lives in frontend UI packages, not in the backend module;
- no FFI crate is planned until a real non-Rust consumer requires one.

All crates may remain in the RusTok monorepo during development. `fly` and
`fly-leptos` must nevertheless be designed as independently publishable projects with
no RusTok dependencies.

## Current verified baseline

- [x] `apps/next-admin/packages/blog/src/components/page-builder.tsx` mounts GrapesJS
  with `grapesjs-preset-webpage`.
- [x] The Next editor loads projects through `loadProjectData()` and saves through
  `getProjectData()`.
- [x] The stored builder contract is `grapesjs_v1`.
- [x] `rustok-page-builder` exists as an FBA capability provider for `preview`, `tree`,
  `properties` and `publish`.
- [x] `rustok-page-builder` owns permission mapping, typed errors, rollout profiles,
  health evidence and transport-neutral endpoint envelopes.
- [x] `rustok-pages` is the reference consumer and declares the `grapesjs_v1`
  dependency in `rustok-module.toml`.
- [x] `rustok-pages/admin` already has `core`, `model`, `transport` and `ui` layers.
- [x] The current Leptos Pages admin can edit `grapesjs_v1` project data as JSON and
  therefore provides a safe fallback while the visual editor is built.
- [x] `modules.toml` is the build-time source of truth for enabled platform modules.
- [ ] Real GrapesJS fixtures and a compatibility matrix are committed.
- [ ] The Fly crates exist.
- [ ] A module contribution registry exists.
- [ ] A frontend control UI for the Page Builder module exists.

## Target architecture

```text
                                  FRONTEND

  module-owned UI package
  core + transport facade + framework UI
        |                     |
        |                     +-- native/server adapter -> backend module
        +------------------------ GraphQL adapter       -> backend module
        |
        v
  fly-ui                         RusTok UI bridge and contribution composition
        |
        v
  fly-leptos                     generic Leptos editor adapter
        |
        v
  fly                            project engine, registries, commands and codec

                                  BACKEND

  rustok-page-builder
        |
        +-- validation / sanitization / preview / publish
        +-- RBAC / tenant scope / rollout / health
        +-- persistence and rendering ports
        +-- may depend on `fly`
        +-- must not depend on `fly-leptos` or `fly-ui`
```

## Dependency rules

```text
fly-leptos -> fly
fly-ui -> fly-leptos -> fly
module admin/storefront UI -> fly-ui and/or fly-leptos -> fly
rustok-page-builder -> fly
```

Forbidden dependencies:

```text
fly -X-> rustok-*
fly-leptos -X-> rustok-*
fly-ui -X-> rustok-pages / rustok-blog / rustok-forum / other domain modules
rustok-page-builder -X-> fly-leptos
rustok-page-builder -X-> fly-ui
```

`fly-ui` is intentionally a RusTok-specific bridge even though its short workspace
name does not include `rustok`. It must not be presented as the generic Fly UI crate.
If it is ever published independently, renaming it to `rustok-fly-ui` should be
considered before release.

## Reference implementation and compatibility contract

### Behavioural reference

GrapesJS is the behavioural reference for the first stable Fly release:

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
| StorageManager | project codec plus host-owned persistence |
| Plugins | `FlyPlugin` |
| component views | framework renderers in `fly-leptos` and later `fly-dioxus` |

Fly should preserve the same capabilities, but its public API must be idiomatic Rust
rather than a literal translation of the JavaScript API.

### Format reference

The first canonical project format remains `grapesjs_v1`. Test fixtures must be
captured from the real Next GrapesJS editor by calling `getProjectData()`.
Hand-written fixture JSON is not sufficient as the only compatibility evidence.

Required round trip:

```text
GrapesJS getProjectData()
  -> Fly deserialize
  -> Fly inspect or mutate
  -> Fly serialize
  -> GrapesJS loadProjectData()
```

The round trip must preserve:

- pages and frames;
- component types, hierarchy, content and attributes;
- styles, selectors, classes and media rules;
- assets;
- traits;
- plugin-specific metadata;
- custom component fields;
- unknown fields introduced by newer GrapesJS versions or third-party plugins.

Unknown data must be preserved even when Fly cannot interpret it.

## Fly ecosystem crates

### `fly`

`fly` is the standalone editor engine. It owns:

- project and document model;
- lossless `grapesjs_v1` codec;
- component tree and mutations;
- editor state and selection;
- commands;
- undo and redo history;
- block, component, trait, style, selector and asset registries;
- plugin registration;
- validation reports;
- missing-plugin and unknown-component preservation;
- generic built-in blocks.

Initial generic blocks should cover at least:

- wrapper, section and container;
- row, columns and grid;
- text, heading, list and link;
- image, video and generic media;
- button;
- divider and spacer;
- basic form, label and input primitives;
- restricted raw HTML where the backend sanitizer permits it.

`fly` must not contain RusTok domain widgets such as forum topics, blog posts,
product cards or cart summaries.

### `fly-leptos`

`fly-leptos` is a generic Leptos adapter. It owns:

- the generic `FlyEditor` component;
- canvas rendering;
- block palette;
- layers tree;
- trait and property panels;
- style controls;
- toolbar and device viewport controls;
- drag and drop;
- selection overlays and resize handles;
- framework renderer registration;
- editor, preview and published render modes;
- missing-plugin placeholders.

A clean Leptos application must be able to use only `fly` and `fly-leptos` without
any RusTok dependency.

### `fly-ui`

`fly-ui` is the RusTok frontend bridge. It owns:

- RusTok design-system bindings;
- host locale and i18n resolver bridging;
- host theme integration;
- permission, read-only and degraded-state mapping;
- standard RusTok errors, banners, dialogs and notifications;
- composition of contributions from enabled module UI packages;
- conversion between generic Fly events and module-owned UI intents.

`fly-ui` must not:

- call GraphQL directly;
- call Leptos server functions directly;
- own page, blog, forum or commerce persistence;
- store domain widget translations;
- contain domain widgets from optional modules;
- become the source of truth for tenant or RBAC policy.

### Later `fly-dioxus`

`fly-dioxus` is deferred until the Fly engine, plugin API and Leptos adapter are
stable. It must reuse the same `fly` model and plugin metadata without duplicating
domain logic.

## Extension and plugin model

Fly owns the extension mechanism, not the full catalogue of business widgets.

A plugin must be able to register:

- blocks shown in the palette;
- component types stored in project data;
- traits and custom property editors;
- editor renderers;
- preview and published renderer descriptors;
- commands;
- optional panels and tools;
- asset providers;
- migrations for its own component schema versions.

Illustrative contract:

```rust
pub trait FlyPlugin {
    fn manifest(&self) -> PluginManifest;
    fn register(&self, registry: &mut FlyRegistry);
}
```

Every custom component identifier must be namespaced and stable, for example:

```text
rustok.forum.latest_topics
rustok.blog.featured_post
rustok.product.product_grid
```

Each stored custom component must carry enough information to diagnose and migrate
it safely:

```json
{
  "type": "rustok.forum.latest_topics",
  "provider": "forum",
  "schemaVersion": 1,
  "traits": {
    "categoryId": "general",
    "limit": 10,
    "sort": "recent"
  }
}
```

Provider-owned migrations must be explicit and versioned. Fly may orchestrate a
migration but must not invent domain migrations for a module.

If the provider plugin is unavailable, Fly must:

- retain the complete node and unknown fields;
- render a missing-plugin placeholder in the editor;
- report the component type and required provider;
- prevent destructive implicit conversion;
- allow safe deletion only as an explicit user action.

## Module-owned widget contract

Module UI packages own their builder widgets. The builder does not store all RusTok
features centrally.

A module contribution may include these roles:

- `editor_host` — mounts a complete Fly editor for a module-owned document or template;
- `block_provider` — contributes palette blocks and component types;
- `trait_provider` — contributes selectors and custom property editors;
- `asset_provider` — contributes asset browsing or selection;
- `editor_renderer` — renders a widget inside the editing canvas;
- `storefront_renderer` — renders the published widget;
- `control_contribution` — contributes diagnostics or policy UI without a visual block.

A typical module UI layout is:

```text
crates/rustok-forum/
  admin/
    locales/
      en.json
      ru.json
    src/
      core/
      model.rs
      transport/
        mod.rs
        graphql_adapter.rs
        native_server_adapter.rs
      builder/
        mod.rs
        plugin.rs
        blocks.rs
        components.rs
        traits.rs
        renderers.rs
      ui/
        leptos/
  storefront/
    src/
      builder/
        renderers.rs
```

All widget translations belong to the contributing module UI package:

- palette category, title and description;
- trait labels and option labels;
- validation and empty-state messages;
- editor preview text;
- published rendering labels where applicable.

Fly owns translations only for generic editor chrome. `fly-ui` owns translations only
for RusTok-wide integration states. A forum widget must never require a forum string
inside Fly or `fly-ui`.

## FFA boundary for every module UI

Each module-owned UI is responsible for its own FFA boundary. The visual widget is
shared while transport is selected by the module frontend adapter.

```text
module widget UI
  -> module transport facade
     -> native/server adapter for embedded SSR/monolith
     -> GraphQL adapter for standalone/headless
  -> module backend service
```

Rules:

- UI components do not branch directly on GraphQL versus server functions;
- `fly`, `fly-leptos` and `fly-ui` never select domain transports;
- both transports return transport-neutral module-owned types;
- native and GraphQL paths must apply identical domain and authorization semantics;
- locale, tenant and auth context come from the host contract;
- widget preview may fetch live module data only through the module transport facade;
- dynamic widgets store query/configuration traits, not a snapshot of resolved domain data;
- a module can use the same widget UI in embedded and headless Leptos profiles.

## Frontend management of the builder

The backend `rustok-page-builder` module currently has `ui_classification =
"capability_only"`. A later phase adds a module-owned admin frontend at:

```text
crates/rustok-page-builder/admin/
  src/
    core/
    model.rs
    transport/
      graphql_adapter.rs
      native_server_adapter.rs
    ui/
      leptos/
```

This is a builder control surface, not the page editor itself.

It will manage through its own FFA adapters:

- installed Fly core and module contribution inventory;
- build-time availability versus tenant runtime enablement;
- plugin compatibility and missing-provider diagnostics;
- global and tenant-level core-block allowlists;
- module-widget visibility policy within the set of enabled modules;
- default presets and starter templates;
- project compatibility and migration diagnostics;
- provider health, degraded mode and rollout state;
- effective capability and permission information;
- links to module-owned widget settings where those settings belong to another module.

It must not:

- edit pages, posts, forum layouts or product layouts directly;
- own domain widgets;
- own module widget translations;
- replace the consumer module UI;
- bypass backend RBAC, rollout or settings services;
- expose widgets from modules that are absent from the build.

When this control UI is implemented, `rustok-page-builder/rustok-module.toml` must be
updated from `capability_only` to the correct UI classification and declare the admin
UI package. The change must be accompanied by manifest and UI package verification.

## Composition and enablement

There are two distinct enablement boundaries.

### Build-time composition

`modules.toml` determines which module code and UI contributions are available in the
build. Contributions from absent modules must not enter the binary or frontend bundle.

### Runtime tenant composition

Within the build-time set, tenant module enablement and builder policy determine which
contributions are visible and usable for a tenant.

The generated contribution registry should therefore compose only build-available
plugins, while the frontend control state filters them by tenant and permissions.

A project containing a component from a currently disabled or unavailable module must
remain lossless and display a diagnostic placeholder. Runtime disablement must not
delete project nodes.

## Module UI support matrix

The matrix defines the planned initial ownership. It is not permission to place all
listed widgets in Fly core.

| Module UI | Planned role | Initial contribution scope | Delivery wave |
|---|---|---|---|
| `rustok-page-builder/admin` | `control_surface` | registry, policies, presets, compatibility, health and rollout management | Control UI phase |
| `rustok-pages/admin` | `editor_host`, `block_provider`, `trait_provider` | page links, menus, reusable sections and full page editing | Pilot |
| `rustok-pages/storefront` | `storefront_renderer` | published page layouts and Pages-owned widgets | Pilot publish |
| `rustok-media/admin` | `asset_provider`, `block_provider` | media picker, image, gallery and video asset integration | Wave A |
| `rustok-blog/admin` | `editor_host`, `block_provider`, `trait_provider` | latest posts, featured post, category feed, author card and blog templates | Wave B |
| `rustok-blog/storefront` | `storefront_renderer` | published blog widgets and templates | Wave B |
| `rustok-forum/admin` | `editor_host`, `block_provider`, `trait_provider` | latest topics, popular discussions, category list, topic feed and forum templates | Wave B |
| `rustok-forum/storefront` | `storefront_renderer` | published forum widgets and templates | Wave B |
| `rustok-product/admin` | `block_provider`, `trait_provider`, optional `editor_host` | product card, product grid, recommendations, category carousel and product templates | Wave B |
| `rustok-product/storefront` | `storefront_renderer` | published product widgets and templates | Wave B |
| `rustok-pricing/admin` | `trait_provider`, `block_provider` | price display configuration and pricing-table widgets | Wave B |
| `rustok-pricing/storefront` | `storefront_renderer` | published price and pricing-table rendering | Wave B |
| `rustok-taxonomy` UI contribution | `trait_provider`, optional `block_provider` | taxonomy selectors, category navigation and taxonomy query configuration | Wave B when a manifest-backed UI contribution exists |
| `rustok-seo/admin` and owner SEO panels | `control_contribution` | SEO inspector and metadata settings; no generic visual block ownership by default | Wave B |
| `rustok-commerce/admin` | `editor_host`, `block_provider` | merchandising layouts, cart summary, checkout CTA and commerce composition | Wave C |
| `rustok-commerce/storefront` | `storefront_renderer` | published commerce widgets | Wave C |
| `rustok-search/admin` | `block_provider`, `trait_provider` | search box, result list and facet configuration | Wave C |
| `rustok-search/storefront` | `storefront_renderer` | published search widgets | Wave C |
| `rustok-comments/admin` | future `block_provider` | comment feed, count and discussion embedding after a published renderer contract exists | Wave C |
| `rustok-profiles` UI contribution | future `block_provider` | member, author and profile widgets after a manifest-backed UI package exists | Wave C |
| `rustok-region` UI packages | `trait_provider` where required | region and availability selectors, not a default block catalogue | As needed |
| `rustok-channel/admin` | `trait_provider` where required | channel targeting and visibility controls | As needed |
| `rustok-workflow/admin` | `control_contribution` | publish workflow/status controls, not domain visual blocks by default | As needed |

Modules not listed as initial providers, including order, payment, fulfillment,
inventory and customer, must not expose privileged operational actions as visual page
blocks by default. Any later contribution requires its own security review, UI ownership,
published-renderer contract and FFA parity tests.

## Runtime flows

### Editing a document

```text
consumer module UI loads project through its transport facade
  -> constructs Fly editor state
  -> fly-ui applies RusTok context and policies
  -> fly-leptos renders the editor
  -> module plugins contribute widgets and editors
  -> Fly emits project-change and UI-intent events
  -> consumer module UI decides when and how to save
```

Fly does not save a page by itself.

### Previewing a dynamic module widget

```text
fly-leptos invokes the registered module editor renderer
  -> module widget UI calls its own transport facade
  -> native or GraphQL adapter loads preview data
  -> widget renders in editor mode
```

### Saving and publishing

```text
consumer module UI
  -> module transport facade
  -> rustok-page-builder capability endpoint where builder validation/publish is required
  -> owner module persistence and lifecycle service
  -> storefront renderer resolves module-owned widgets
```

The backend remains authoritative for permissions, sanitization, tenant scope,
idempotency and publish state.

### Missing module

```text
project contains rustok.forum.latest_topics
  -> forum contribution unavailable
  -> Fly retains the raw node
  -> editor shows a missing-provider placeholder
  -> save remains lossless
  -> explicit deletion is allowed only by user action
```

## Project and widget versioning

- `grapesjs_v1` remains the outer project contract during the compatibility period.
- Fly crate versions and project-contract versions are separate concerns.
- Core Fly block identifiers are stable after the first compatibility release.
- Each module widget has a stable namespaced type and independent schema version.
- Provider modules own widget migrations.
- Migration must be explicit, observable and reversible where possible.
- A project must record diagnostics when a widget schema is newer than the installed
  provider understands.
- Generated HTML/CSS may be cached as a derived publish artifact but must not replace
  canonical project data.

## Security and operational requirements

The following requirements are part of the plan and must not be deferred as implicit
implementation details:

- arbitrary component scripts are disabled by default;
- raw HTML, URLs, attributes and CSS pass backend sanitization policy;
- editor preview is not treated as trusted published output;
- dynamic widgets cannot bypass module RBAC through frontend preview calls;
- asset selection integrates with module-owned media permissions;
- tenant context is mandatory for module widget data access;
- publish writes require existing deadline and idempotency semantics;
- missing or disabled plugins never cause silent data deletion;
- widget dependency cycles are rejected by registry validation;
- plugin and renderer panics/errors become typed diagnostics, not editor-wide corruption;
- published dynamic widgets define cache keys and invalidation ownership;
- editor and storefront renderers must have parity tests for significant states;
- accessibility and keyboard interaction are part of the generic editor acceptance gate;
- project size limits and history limits must be configurable to prevent browser memory
  exhaustion.

## Implementation phases

### Phase 0 — Baseline, ADR and compatibility evidence

- [ ] **Phase status:** in progress.
- [x] Keep the current Next GrapesJS editor operational as the reference editor.
- [x] Keep `grapesjs_v1` as the current backend and consumer contract.
- [x] Preserve the existing `rustok-page-builder` FBA provider boundary.
- [x] Preserve the Pages JSON editor as a fallback.
- [ ] Add an ADR recording the Fly ownership and dependency decisions.
- [ ] Capture real GrapesJS fixtures for basic page, multi-page, styles/selectors,
  assets, traits, custom components and plugin metadata.
- [ ] Record the GrapesJS version and enabled plugins used to generate each fixture.
- [ ] Add a Node-based compatibility harness that reloads Fly-produced fixtures through
  `loadProjectData()`.
- [ ] Record the module UI support matrix in module-local plans before implementation
  begins in each module.
- [ ] Add a source-level guard that rejects RusTok dependencies from `fly` and
  `fly-leptos`.

**Phase gate:** fixtures and compatibility expectations are reproducible in CI, and the
reference Next editor remains unchanged in behaviour.

### Phase 1 — `fly` lossless project model and codec

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly` with its own README, docs and implementation plan.
- [ ] Implement a raw lossless project representation.
- [ ] Implement typed accessors for pages, frames, components, styles, selectors,
  assets and traits.
- [ ] Preserve unknown fields at every extensible level.
- [ ] Implement deterministic serialization without requiring semantic normalization.
- [ ] Implement traversal and validation reports.
- [ ] Add property-based and fixture round-trip tests.
- [ ] Confirm no dependency on any `rustok-*`, Leptos or backend crate.

**Phase gate:** all captured GrapesJS fixtures complete a lossless
GrapesJS -> Fly -> GrapesJS round trip.

### Phase 2 — Fly editor engine and plugin API

- [ ] **Phase status:** not started.
- [ ] Implement `FlyEditor` state and selection.
- [ ] Implement command execution and undo/redo history.
- [ ] Implement component, block, trait, style, selector and asset registries.
- [ ] Implement `FlyPlugin` and plugin manifest contracts.
- [ ] Implement stable namespaced component IDs and schema versions.
- [ ] Implement unknown-component and missing-provider handling.
- [ ] Implement initial generic block set.
- [ ] Implement plugin dependency and duplicate-ID validation.
- [ ] Add tests proving that projects can be built and mutated only through the Fly API.

**Phase gate:** a non-visual Rust test can construct, edit, undo, redo, serialize and
reload a project containing both core and custom plugin components.

### Phase 3 — Backend Fly integration

- [ ] **Phase status:** not started.
- [ ] Add a dependency from `rustok-page-builder` to `fly` only.
- [ ] Replace synthetic tree inspection with Fly traversal.
- [ ] Route project validation through Fly while preserving the current typed error
  catalogue.
- [ ] Connect Fly-backed preview/rendering behind the existing rendering adapter seam.
- [ ] Preserve the public FBA request/response envelopes and capability names.
- [ ] Keep sanitization and authorization authoritative in the backend module.
- [ ] Add runtime tests using projects saved by the Next GrapesJS editor.

**Phase gate:** the current Next editor can save a project that the backend validates,
inspects, previews and publishes through Fly without a public contract break.

### Phase 4 — Generic `fly-leptos`

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly-leptos` with no RusTok dependencies.
- [ ] Implement the generic editor shell, canvas, layers and block palette.
- [ ] Implement selection, drag/drop, resize and keyboard interaction.
- [ ] Implement generic trait, style and asset panels.
- [ ] Implement framework renderer registration for plugin components.
- [ ] Implement editor, preview and published render modes.
- [ ] Implement the missing-plugin placeholder.
- [ ] Add an independent example application outside RusTok module UI packages.
- [ ] Add accessibility and browser interaction tests.

**Phase gate:** a clean Leptos application can embed a working Fly editor using only
`fly` and `fly-leptos`.

### Phase 5 — RusTok `fly-ui` bridge and generated contribution registry

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly-ui` as a RusTok-specific bridge.
- [ ] Integrate RusTok UI primitives, theme and host context.
- [ ] Integrate the framework-neutral i18n core through the Leptos i18n adapter.
- [ ] Map permissions, read-only mode and degraded capabilities into Fly UI state.
- [ ] Define the module UI contribution contract.
- [ ] Extend module manifest metadata for builder contributions.
- [ ] Generate the build-time contribution registry from enabled modules.
- [ ] Filter build-available contributions by tenant runtime state.
- [ ] Verify that `fly-ui` has no direct dependency on optional domain modules.
- [ ] Verify that `fly-ui` performs no GraphQL or server-function calls.

**Phase gate:** a generated RusTok registry composes only enabled module contributions,
and the bridge can mount them without owning their widgets or transports.

### Phase 6 — Frontend Page Builder control UI

- [ ] **Phase status:** not started.
- [ ] Create `crates/rustok-page-builder/admin` with `core/model/transport/ui` FFA
  structure.
- [ ] Add native/server and GraphQL adapters with semantic parity.
- [ ] Expose registry inventory, compatibility diagnostics and provider health.
- [ ] Expose core-block allowlists and module-widget visibility policy.
- [ ] Expose presets and starter-template management.
- [ ] Expose migration diagnostics and explicit migration actions.
- [ ] Show effective rollout, degraded-state and permission information.
- [ ] Update `rustok-page-builder/rustok-module.toml` UI classification and admin UI
  wiring.
- [ ] Add manifest, FFA and headless parity verification.

**Phase gate:** the same module-owned control UI works in embedded Leptos and
headless-compatible profiles, and all mutations still pass through backend policy and
RBAC.

### Phase 7 — Pages visual-editor pilot

- [ ] **Phase status:** not started.
- [ ] Add a Pages-owned builder contribution package inside `rustok-pages/admin`.
- [ ] Add Pages blocks, traits, editor renderers and translations.
- [ ] Mount `fly-ui`/`fly-leptos` in the existing Pages admin UI.
- [ ] Keep the JSON textarea as an explicit debug/fallback surface during rollout.
- [ ] Keep page metadata and lifecycle ownership in `rustok-pages`.
- [ ] Keep native/server and GraphQL paths behind the Pages transport facade.
- [ ] Add Next GrapesJS <-> Leptos Fly cross-editor round-trip tests.
- [ ] Add tenant fallback tests for `all_on`, `publish_off`, `preview_off` and
  `builder_off`.

**Phase gate:** the same page can be opened, edited and saved alternately by the Next
GrapesJS editor and Leptos Fly without loss or lifecycle regression.

### Phase 8 — Module contribution rollout

- [ ] **Phase status:** not started.

#### Wave A — platform foundations

- [ ] `rustok-media/admin`: asset provider and media widgets.
- [ ] Pages-owned reusable sections and menus.
- [ ] Generated contribution and i18n completeness verification.

#### Wave B — primary content and commerce entities

- [ ] `rustok-blog` admin/storefront contributions.
- [ ] `rustok-forum` admin/storefront contributions.
- [ ] `rustok-product` admin/storefront contributions.
- [ ] `rustok-pricing` admin/storefront contributions.
- [ ] taxonomy selector contribution after a manifest-backed UI owner is established.
- [ ] owner-module SEO inspector contributions.

#### Wave C — composite and discovery widgets

- [ ] `rustok-commerce` admin/storefront contributions.
- [ ] `rustok-search` admin/storefront contributions.
- [ ] comments contribution after a published-renderer contract exists.
- [ ] profiles contribution after a manifest-backed UI package exists.

For every contributing module:

- [ ] plugin IDs and schema versions are documented;
- [ ] translations live in the module UI package;
- [ ] editor renderer and published renderer are owned by the module;
- [ ] native and GraphQL FFA adapters have parity tests;
- [ ] missing-provider preservation is tested;
- [ ] local module docs and implementation plans are updated;
- [ ] security and cache ownership are recorded.

**Phase gate:** disabling a module removes its contributions from the active palette
without deleting existing project nodes, and each enabled module passes editor/published
renderer parity.

### Phase 9 — Published rendering and rollout completion

- [ ] **Phase status:** not started.
- [ ] Define a shared versioned widget configuration contract between admin and
  storefront packages.
- [ ] Compile and cache safe derived HTML/CSS where useful without replacing project
  data.
- [ ] Complete module-owned storefront renderers.
- [ ] Define cache keys and invalidation events for dynamic widgets.
- [ ] Correlate editor save -> builder publish -> owner lifecycle -> storefront read.
- [ ] Replace synthetic Wave evidence with observed tenant packets.
- [ ] Complete rollback and legacy-block bridge exit criteria.

**Phase gate:** preview and storefront output have verified parity for pilot modules,
and a tenant rollout can be promoted or rolled back without redeploying.

### Phase 10 — Dioxus adapter

- [ ] **Phase status:** deferred.
- [ ] Create `fly-dioxus` only after Fly and plugin contracts are stable.
- [ ] Reuse module core models and contribution metadata.
- [ ] Add Dioxus-specific renderers without duplicating domain or transport semantics.
- [ ] Verify behaviour against the Leptos adapter.

**Phase gate:** at least one complex module contribution works through both Leptos and
Dioxus adapters with the same project data and domain behaviour.

### Phase 11 — Optional repository extraction

- [ ] **Phase status:** deferred.
- [ ] Confirm `fly` and `fly-leptos` have no RusTok dependency leakage.
- [ ] Stabilize public API and semantic versioning.
- [ ] Decide license and publication policy.
- [ ] Extract to a separate repository only when independent release cadence provides
  more value than monorepo development.

**Phase gate:** extraction requires no RusTok code refactor, only workspace and release
wiring changes.

## Verification programme

Expected commands as crates and scripts are introduced:

```text
cargo test -p fly
cargo test -p fly-leptos
cargo test -p fly-ui
cargo test -p rustok-page-builder
cargo test -p rustok-page-builder-admin
cargo test -p rustok-pages-admin
cargo xtask module validate page_builder
cargo xtask module validate pages
npm run verify:page-builder:fba:baseline
npm run verify:i18n:ui
npm run verify:i18n:contract
```

New verification suites must cover:

- GrapesJS -> Fly -> GrapesJS fixture round trips;
- Node reload through `loadProjectData()`;
- unknown field and unknown plugin preservation;
- duplicate component and plugin IDs;
- plugin dependency validation;
- component schema migration;
- undo/redo correctness;
- build-time module composition;
- runtime tenant filtering;
- native/server and GraphQL FFA parity;
- editor and storefront renderer parity;
- missing-provider fallback;
- sanitization and script rejection;
- asset permission boundaries;
- i18n completeness for every module contribution;
- accessibility and keyboard operation;
- project-size and history limits;
- publish idempotency and rollback.

## Previously omitted concerns now captured

The implementation must explicitly account for:

- the distinction between a full editor host and a widget-only provider;
- a frontend control UI for the Page Builder backend module;
- build-time composition versus tenant runtime enablement;
- module-owned translations;
- module-owned editor and storefront renderers;
- provider-owned widget migrations;
- unknown widget preservation;
- dynamic widget data access through module FFA adapters;
- asset-provider ownership;
- cache and invalidation ownership;
- security restrictions on scripts and privileged operational widgets;
- accessibility and browser memory limits;
- the continued role of Next GrapesJS as compatibility reference during migration.

## Update rules

- This document is the central cross-module Fly programme plan.
- `crates/rustok-page-builder/docs/implementation-plan.md` remains the backend-provider
  plan and must link to this programme when implementation starts.
- Every contributing module must update its own `docs/implementation-plan.md` before its
  rollout task is marked complete here.
- Contract changes require matching verification-script changes in the same iteration.
- Phase checkboxes are updated only from merged code and reproducible evidence.
- After each phase, search for and remove outdated wording that presents the builder as
  either GrapesJS-only or backend-owned UI.
