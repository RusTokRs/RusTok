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

The final layer split is:

- `fly` — framework-neutral editor engine, project model, registries, commands, history and
  lossless GrapesJS compatibility;
- `fly-ui` — framework-neutral visual-editor UI model and contracts shared by all framework
  adapters and deployment surfaces;
- `fly-leptos` — the Leptos implementation of `fly-ui`, including DOM interaction, canvas,
  drag-and-drop, overlays and framework renderer factories;
- `rustok-page-builder/admin` — the optional Page Builder admin UI package for full authoring
  and builder control-plane management;
- `rustok-page-builder/storefront` — the optional Page Builder storefront UI package for
  in-context editing on public frontends;
- module-owned admin and storefront UI packages — providers of domain blocks, widgets,
  traits, translations, editor renderers, published renderers and their own FFA adapters;
- `rustok-page-builder` — the backend FBA provider for validation, sanitization, preview,
  publish, rollout, permissions, persistence and rendering seams.

`fly-ui` is not a RusTok bridge, transport layer or module control surface. It answers how a
visual editor behaves independently from Leptos, Dioxus, admin routing, storefront routing,
GraphQL, server functions and RusTok deployment topology.

The Page Builder module has two classic UI packages because admin and storefront are separate
deployment surfaces. They share the Fly libraries but retain independent routes, security,
transport facades, permissions, bundles and release profiles.

If the Page Builder module is absent from a build, visual editing, palettes, drag-and-drop and
builder management surfaces are absent. Consumer modules keep their normal CRUD, rendering and
documented fallback paths, and canonical project data is not deleted or rewritten.

## Current verified baseline

- [x] `apps/next-admin/packages/blog/src/components/page-builder.tsx` mounts GrapesJS with
  `grapesjs-preset-webpage`.
- [x] The Next editor loads projects through `loadProjectData()` and saves through
  `getProjectData()`.
- [x] The stored builder contract is `grapesjs_v1`.
- [x] `rustok-page-builder` exists as an FBA capability provider for `preview`, `tree`,
  `properties` and `publish`.
- [x] `rustok-page-builder` owns permission mapping, typed errors, rollout profiles, health
  evidence and transport-neutral endpoint envelopes.
- [x] `rustok-pages` is the reference consumer and declares the `grapesjs_v1` dependency.
- [x] `rustok-pages/admin` already has `core`, `model`, `transport` and `ui` layers.
- [x] The current Leptos Pages admin can edit `grapesjs_v1` data as JSON and provides a safe
  fallback while the visual editor is built.
- [x] `modules.toml` is the build-time source of truth for enabled platform modules.
- [x] The programme plan records the separate roles of `fly-ui`, `fly-leptos`, Page Builder
  admin and Page Builder storefront.
- [ ] Real GrapesJS fixtures and a compatibility matrix are committed.
- [ ] The `fly`, `fly-ui` and `fly-leptos` crates exist.
- [ ] `rustok-page-builder/admin` exists as a manifest-backed UI package.
- [ ] `rustok-page-builder/storefront` exists as a manifest-backed UI package.
- [ ] Generated admin and storefront contribution registries exist.

## Target architecture

```text
                           EXTERNAL / REUSABLE FLY LAYERS

  fly
  engine, project model, registries, commands, history, codec
    ^
    |
  fly-ui
  framework-neutral visual-editor state, intents, policies and UI contracts
    ^
    |
  fly-leptos
  Leptos DOM, signals, canvas, DnD, overlays and renderer factories
    ^                                      ^
    |                                      |
    |                                      |
  rustok-page-builder/admin          rustok-page-builder/storefront
  full authoring + control UI        in-context editing + preview overlays
    ^                                      ^
    |                                      |
  module admin contributions         module storefront contributions
  editor widgets and traits          published renderers and inline edit integration

                                  BACKEND

  rustok-page-builder
    +-- validation / sanitization / preview / publish
    +-- RBAC / tenant scope / rollout / health
    +-- persistence and rendering ports
    +-- may depend on `fly`
    +-- must not depend on `fly-ui`, `fly-leptos`, admin or storefront UI
```

Hosts remain technical composition roots only. They may mount module-owned surfaces, provide
runtime context and include generated contribution factories. They must not own editor
behaviour, widget schemas, translations, transport selection or persistence semantics.

## Physical package layout

```text
crates/
  fly/                                  # standalone engine
  fly-ui/                               # standalone framework-neutral UI contracts
  fly-leptos/                           # standalone Leptos implementation

  rustok-page-builder/
    src/                                # backend FBA provider

    admin/                              # optional admin deployment surface
      locales/
        en.json
        ru.json
      src/
        core.rs
        model.rs
        transport/
          mod.rs
          graphql_adapter.rs
          native_server_adapter.rs
        editor/
          full_editor.rs
          admin_shell.rs
        control/
          registry.rs
          policies.rs
          presets.rs
          compatibility.rs
          health.rs
        ui/
          leptos.rs

    storefront/                         # optional storefront deployment surface
      locales/
        en.json
        ru.json
      src/
        core.rs
        model.rs
        transport/
          mod.rs
          graphql_adapter.rs
          native_server_adapter.rs
        editor/
          inline_editor.rs
          edit_overlay.rs
          draft_preview.rs
        ui/
          leptos.rs

  rustok-pages/
    admin/src/builder/                  # Pages admin/editor contribution
    storefront/src/builder/             # Pages published/inline contribution

  rustok-forum/
    admin/src/builder/                  # Forum editor contribution
    storefront/src/builder/             # Forum published/inline contribution
```

There is no third shared RusTok UI package by default. Shared editor mechanics belong in
`fly-ui` and `fly-leptos`. A later Page Builder support crate is justified only by concrete
RusTok-specific duplication that cannot correctly live in either deployment package.

## Dependency rules

```text
fly-ui -> fly
fly-leptos -> fly-ui + fly

rustok-page-builder-admin -> fly-leptos -> fly-ui -> fly
rustok-page-builder-storefront -> fly-leptos -> fly-ui -> fly

module admin contribution -> fly / fly-ui / fly-leptos contribution contracts
module storefront contribution -> fly / fly-ui / fly-leptos contribution contracts

rustok-page-builder backend -> fly
```

Generated registries may depend on enabled module UI contribution factories and pass them into
the Page Builder admin or storefront package. The Page Builder UI packages must not hard-code
direct dependencies on every optional domain module.

Forbidden dependencies:

```text
fly -X-> leptos / dioxus / rustok-*
fly-ui -X-> leptos / dioxus / rustok-*
fly-leptos -X-> rustok-*

rustok-page-builder backend -X-> fly-ui / fly-leptos / admin / storefront
rustok-page-builder-admin -X-> optional domain admin packages directly
rustok-page-builder-storefront -X-> optional domain storefront packages directly
module contribution -X-> host application code
```

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
| StorageManager | project codec plus consumer-owned persistence |
| Plugins | `FlyPlugin` |
| component views | framework renderers in `fly-leptos`, later other adapters |

Fly preserves the capabilities while exposing idiomatic Rust APIs rather than a literal
translation of the JavaScript API.

### Format reference

The first canonical project format remains `grapesjs_v1`. Test fixtures must be captured from
the real Next GrapesJS editor through `getProjectData()`. Hand-written JSON is not sufficient
as the only compatibility evidence.

Required round trip:

```text
GrapesJS getProjectData()
  -> Fly deserialize
  -> Fly inspect or mutate
  -> Fly serialize
  -> GrapesJS loadProjectData()
```

The round trip preserves:

- pages and frames;
- component types, hierarchy, content and attributes;
- styles, selectors, classes and media rules;
- assets;
- traits;
- plugin-specific metadata;
- custom component fields;
- unknown fields introduced by newer GrapesJS versions or third-party plugins.

Unknown data remains lossless even when Fly cannot interpret it.

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
- missing-plugin and unknown-component preservation;
- generic built-in blocks.

Initial generic blocks include wrapper, section, container, rows, columns, grid, text, heading,
list, link, image, video, generic media, button, divider, spacer, basic form primitives and
restricted raw HTML where backend policy permits it.

`fly` does not contain RusTok domain widgets such as forum topics, blog posts, product cards or
cart summaries.

### `fly-ui`

`fly-ui` is a standalone framework-neutral library. It owns the reusable visual-editor model:

- presentation modes: full, inline, preview and read-only;
- editor layout and panel-state models;
- palette, layers, traits, styles and asset-panel contracts;
- toolbar and viewport action contracts;
- editor intents and command-facing UI actions;
- selection and overlay models;
- drag-and-drop intent model independent from DOM events;
- property-editor and component-renderer contracts;
- contribution registry contracts;
- editor policies and feature capability model;
- generic UI message identifiers and framework-neutral accessibility metadata.

Illustrative contracts:

```rust
pub enum EditorPresentation {
    Full,
    Inline,
    Preview,
    ReadOnly,
}

pub enum EditorIntent {
    ProjectChanged(FlyProject),
    PreviewRequested,
    SaveRequested,
    PublishRequested,
    ExitRequested,
}
```

`fly-ui` must not contain:

- Leptos or Dioxus components;
- DOM, browser or server runtime code;
- RusTok theme or routing;
- GraphQL or server functions;
- tenant loading or RBAC implementation;
- Pages, Blog, Forum or Commerce persistence;
- module-specific widgets or translations.

### `fly-leptos`

`fly-leptos` implements `fly-ui` for Leptos. It owns:

- DOM and signal integration;
- canvas rendering;
- generic palette, layers and panel components;
- pointer and keyboard interaction;
- drag-and-drop and resize handles;
- selection and inline-edit overlays;
- Leptos renderer and property-editor factories;
- viewport/device controls;
- missing-plugin placeholders;
- browser accessibility and interaction behaviour.

A clean Leptos application must be able to build a Fly editor using only `fly`, `fly-ui` and
`fly-leptos`, without importing RusTok.

### Later framework adapters

A later `fly-dioxus` implements the same `fly-ui` contracts without duplicating engine or
domain behaviour. Framework adapters may add framework-specific factories but must not fork
the Fly UI state model or project semantics.

## Page Builder UI packages

### `rustok-page-builder/admin`

The admin package is optional and deployable independently from storefronts. It owns:

- full-screen authoring shell;
- complete palette/layers/traits/styles/assets workspace composition;
- admin navigation and routes;
- Page Builder control surface;
- registry inventory and compatibility diagnostics;
- block allowlists and widget visibility policy;
- presets and starter-template management;
- migration actions;
- provider health, degraded mode and rollout state;
- admin-specific permissions and notifications;
- its own FFA facade with native/server and GraphQL adapters.

It mounts the Leptos Fly editor in `EditorPresentation::Full` mode. It does not own consumer
document persistence and does not call Pages, Blog, Forum or Commerce transports.

### `rustok-page-builder/storefront`

The storefront package is optional and can be deployed on any number of frontend servers. One
crate may be compiled or deployed for several sites with different endpoint, tenant, theme and
registry configuration.

It owns:

- authenticated edit-mode activation;
- in-context visual editing over the real storefront;
- inline toolbar and selection overlays;
- block insertion and drag-and-drop controls;
- draft/published switching;
- preview and exit-edit flows;
- storefront-specific permissions and notifications;
- its own FFA facade with GraphQL and optional native/server adapters.

It mounts the Leptos Fly editor in `EditorPresentation::Inline`, `Preview` or `ReadOnly` mode.
It must not include admin control-plane screens.

### Deployment examples

```text
admin.example.com
  -> rustok-page-builder-admin
  -> native/server or GraphQL adapter

site-a.example.com
site-b.example.com
site-c.example.com
  -> the same rustok-page-builder-storefront crate
  -> separate runtime configuration and usually GraphQL adapters
```

These are four deployment instances but only two Page Builder UI package implementations.

## Consumer ownership and runtime flows

A consumer module owns the document lifecycle. Fly and Page Builder surfaces emit intents but
do not decide how a Page, Post, Forum layout or Product template is persisted.

### Admin editing

```text
consumer admin UI loads document through its own FFA facade
  -> mounts rustok-page-builder-admin full editor
  -> admin registry contributes module editor widgets
  -> Fly emits project and action intents
  -> consumer admin UI saves through its own FFA facade
```

### Storefront in-context editing

```text
consumer storefront UI loads published/draft document
  -> activates rustok-page-builder-storefront edit mode
  -> storefront registry contributes published and inline-edit renderers
  -> Fly emits project and action intents
  -> consumer storefront UI saves through its own FFA facade
```

### Dynamic widget preview

```text
fly-leptos invokes the registered module renderer
  -> module widget UI calls its own module transport facade
  -> native or GraphQL adapter loads preview data
  -> renderer displays editor or storefront state
```

### Save and publish

```text
consumer UI
  -> consumer module transport facade
  -> Page Builder capability endpoint where validation/publish is required
  -> owner module persistence and lifecycle service
  -> module-owned storefront renderer resolves published widgets
```

Backend policy remains authoritative for permissions, sanitization, tenant scope, idempotency
and publish state.

## Module-owned widget contract

Module UI packages own their builder contributions. Fly and Page Builder packages do not store
all RusTok features centrally.

A module contribution may include:

- `document_consumer` — owns a document/template lifecycle and mounts a Page Builder surface;
- `block_provider` — contributes palette blocks and component types;
- `trait_provider` — contributes selectors and custom property editors;
- `asset_provider` — contributes asset browsing or selection;
- `editor_renderer` — renders a component in admin/full editor mode;
- `storefront_renderer` — renders the published component;
- `inline_editor` — adds safe in-context editing integration around a storefront renderer;
- `control_contribution` — contributes diagnostics or policy UI without owning a visual block.

A typical module layout is:

```text
crates/rustok-forum/
  admin/
    locales/
    src/
      core.rs
      model.rs
      transport/
        graphql_adapter.rs
        native_server_adapter.rs
      builder/
        plugin.rs
        blocks.rs
        components.rs
        traits.rs
        editor_renderers.rs
      ui/
        leptos.rs

  storefront/
    locales/
    src/
      transport/
        graphql_adapter.rs
        native_server_adapter.rs
      builder/
        published_renderers.rs
        inline_edit.rs
      ui/
        leptos.rs
```

Shared widget configuration schemas may live in a framework-neutral module-owned core/support
crate when both admin and storefront need them. UI components and transports remain in their
respective UI packages.

All widget translations belong to the contributing module UI surface:

- admin palette, trait labels, editor validation and preview messages live in module admin;
- published and inline-edit labels live in module storefront;
- shared message identifiers may live in module-owned framework-neutral contracts;
- Fly owns only generic engine/UI identifiers;
- Page Builder admin/storefront own only their generic surface strings.

## FFA boundaries

Every deployable UI package has its own FFA boundary.

```text
page-builder admin UI
  -> page-builder admin facade
     -> native/server adapter
     -> GraphQL adapter
  -> page-builder backend

page-builder storefront UI
  -> page-builder storefront facade
     -> GraphQL adapter
     -> optional native/server adapter
  -> page-builder backend

module widget UI
  -> module-specific facade
     -> native/server adapter
     -> GraphQL adapter
  -> module backend
```

Rules:

- components do not branch directly on GraphQL versus server functions;
- `fly`, `fly-ui` and `fly-leptos` never select RusTok transports;
- admin and storefront Page Builder facades are separate;
- module widget data does not flow through the Page Builder facade;
- both transport implementations return transport-neutral package-owned models;
- native and GraphQL paths apply equivalent authorization and domain semantics;
- locale, tenant and auth context come from host contracts;
- dynamic widgets store query/configuration traits, not resolved domain snapshots.

## Composition and enablement

### Build-time composition

`modules.toml` determines which modules and UI surfaces enter a build.

Possible profiles include:

```text
no Page Builder:
  no admin editor
  no storefront edit mode
  no drag-and-drop

admin-only Page Builder:
  rustok-page-builder/admin
  no storefront edit mode

storefront-only Page Builder:
  rustok-page-builder/storefront
  no admin authoring/control UI

full Page Builder:
  rustok-page-builder/admin
  rustok-page-builder/storefront
```

If Page Builder is absent, consumer CRUD and published rendering remain available where the
consumer supports them. Existing project nodes remain untouched.

### Runtime tenant composition

Within build-available surfaces, tenant module state, permissions and Page Builder policy
filter the active contributions. Disabled or unavailable providers produce diagnostics and
placeholders rather than destructive project conversion.

### Separate generated registries

Admin and storefront require different registries:

```text
enabled admin contribution factories
  -> generated admin registry
  -> rustok-page-builder-admin

enabled storefront contribution factories
  -> generated storefront registry
  -> rustok-page-builder-storefront
```

The admin registry includes blocks, traits and editor renderers. The storefront registry
includes published renderers and optional inline-edit integration. A module may contribute to
one surface without contributing to the other.

Host wiring is mechanical only:

```text
enabled factories -> generated registry -> surface context -> Page Builder surface
```

Hosts must not define schemas, translate widgets, select widget transports, implement editor
commands, own builder policy or save consumer documents.

## Module UI support matrix

| Module UI | Planned role | Initial contribution scope | Delivery wave |
|---|---|---|---|
| `rustok-page-builder/admin` | full authoring, `control_surface`, core block presentation | admin editor, registry, policies, presets, compatibility, health, rollout | Admin surface phase |
| `rustok-page-builder/storefront` | inline authoring surface | in-context editing, overlays, draft preview | Storefront surface phase |
| `rustok-pages/admin` | `document_consumer`, `block_provider`, `trait_provider` | page editing, links, menus, reusable sections | Pilot |
| `rustok-pages/storefront` | `storefront_renderer`, optional `inline_editor` | published layouts and Pages-owned widgets | Pilot |
| `rustok-media/admin` | `asset_provider`, `block_provider` | media picker, image, gallery and video integration | Wave A |
| `rustok-media/storefront` or owner renderers | published media rendering | safe media output and edit overlays where needed | Wave A |
| `rustok-blog/admin` | `document_consumer`, `block_provider`, `trait_provider` | blog templates, latest posts, featured post, category feed, author card | Wave B |
| `rustok-blog/storefront` | `storefront_renderer`, optional `inline_editor` | published blog widgets and in-context editing | Wave B |
| `rustok-forum/admin` | `document_consumer`, `block_provider`, `trait_provider` | forum templates, latest topics, popular discussions, category list | Wave B |
| `rustok-forum/storefront` | `storefront_renderer`, optional `inline_editor` | published forum widgets and in-context editing | Wave B |
| `rustok-product/admin` | `block_provider`, `trait_provider`, optional `document_consumer` | product card/grid, recommendations, categories, templates | Wave B |
| `rustok-product/storefront` | `storefront_renderer`, optional `inline_editor` | published product widgets | Wave B |
| `rustok-pricing/admin` | `trait_provider`, `block_provider` | price display and pricing-table configuration | Wave B |
| `rustok-pricing/storefront` | `storefront_renderer` | published price and pricing-table output | Wave B |
| taxonomy owner UI | `trait_provider`, optional `block_provider` | taxonomy selectors and query configuration | Wave B after manifest-backed ownership |
| owner SEO panels and `rustok-seo/admin` support | `control_contribution` | SEO inspector and metadata settings | Wave B |
| `rustok-commerce/admin` | `document_consumer`, `block_provider` | merchandising layouts and commerce composition | Wave C |
| `rustok-commerce/storefront` | `storefront_renderer`, optional `inline_editor` | published commerce widgets | Wave C |
| `rustok-search/admin` | `block_provider`, `trait_provider` | search box, results and facet configuration | Wave C |
| `rustok-search/storefront` | `storefront_renderer` | published search widgets | Wave C |
| `rustok-comments/admin` | future `block_provider` | comment feed/count after published contract exists | Wave C |
| profiles owner UI | future `block_provider` | member, author and profile widgets | Wave C |
| `rustok-region` UI packages | `trait_provider` where required | region and availability selectors | As needed |
| `rustok-channel/admin` | `trait_provider` where required | channel targeting and visibility | As needed |
| `rustok-workflow/admin` | `control_contribution` | publish workflow and status controls | As needed |

Order, Payment, Fulfillment, Inventory and Customer must not expose privileged operational
actions as visual page blocks by default. Any later contribution requires a security review,
clear UI ownership, published-renderer contract and FFA parity tests.

## Plugin, project and widget versioning

A Fly plugin registers blocks, component types, traits, property editors, editor renderers,
published renderer descriptors, commands, optional panels, asset providers and its own schema
migrations.

Custom identifiers are namespaced and stable:

```text
rustok.forum.latest_topics
rustok.blog.featured_post
rustok.product.product_grid
```

Stored custom nodes carry provider and schema information:

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

Provider modules own their migrations. Fly may orchestrate migrations but does not invent
domain transformations.

When a provider is unavailable, Fly retains the complete node and unknown fields, reports the
required provider, renders a placeholder when an editor is available and allows deletion only
as an explicit user action.

`grapesjs_v1` remains the outer project contract during compatibility. Fly versions, UI
versions and widget schema versions are separate. Generated HTML/CSS may be cached as derived
publish output but never replaces canonical project data.

## Security and operational requirements

- Arbitrary component scripts are disabled by default.
- Raw HTML, URLs, attributes and CSS pass backend sanitization.
- Editor preview is not trusted published output.
- Storefront edit mode requires explicit authentication and authorization.
- Dynamic widgets cannot bypass module RBAC through preview calls.
- Asset selection respects module-owned media permissions.
- Tenant context is mandatory for widget data access.
- Publish writes require deadlines and idempotency.
- Missing or disabled plugins never cause silent deletion.
- Disabling either Page Builder surface never converts canonical project data.
- Widget dependency cycles are rejected by registry validation.
- Renderer failures become typed diagnostics rather than editor corruption.
- Published dynamic widgets define cache keys and invalidation ownership.
- Admin editor, storefront editor and published renderer states have parity tests.
- Accessibility and keyboard interaction are acceptance requirements.
- Project-size and history limits are configurable.
- Storefront edit assets and code are excluded from anonymous bundles when the feature profile
  does not enable them.

## Implementation phases

### Phase 0 — Baseline, ADR and compatibility evidence

- [ ] **Phase status:** in progress.
- [x] Keep the current Next GrapesJS editor operational as the reference editor.
- [x] Keep `grapesjs_v1` as the current backend and consumer contract.
- [x] Preserve the existing `rustok-page-builder` FBA provider boundary.
- [x] Preserve the Pages JSON editor as a fallback.
- [x] Record the final four-layer frontend split in this programme.
- [ ] Add an ADR for `fly`, `fly-ui`, `fly-leptos`, admin and storefront ownership.
- [ ] Capture real GrapesJS fixtures for pages, styles, assets, traits and custom plugins.
- [ ] Record the GrapesJS version and plugins for every fixture.
- [ ] Add a Node compatibility harness using `loadProjectData()`.
- [ ] Record the module support matrix in module-local plans.
- [ ] Add dependency guards for `fly`, `fly-ui` and `fly-leptos`.

**Phase gate:** compatibility evidence is reproducible in CI and the ownership split is
captured by ADR.

### Phase 1 — `fly` engine and lossless codec

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly` with README, docs and implementation plan.
- [ ] Implement the lossless project representation and typed accessors.
- [ ] Preserve unknown fields at extensible levels.
- [ ] Implement deterministic serialization, traversal and validation.
- [ ] Implement editor state, mutations, commands and history.
- [ ] Implement registries, plugins, stable IDs and missing-provider handling.
- [ ] Implement the initial generic block set.
- [ ] Add property-based, fixture and command/history tests.
- [ ] Confirm no UI framework or RusTok dependency.

**Phase gate:** fixtures round-trip losslessly and a non-visual Rust test constructs, edits,
undoes, redoes and reloads core and plugin components.

### Phase 2 — Framework-neutral `fly-ui`

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly-ui` with no Leptos, Dioxus or RusTok dependencies.
- [ ] Define presentation modes and editor intents.
- [ ] Define layout, panel, toolbar, selection and overlay models.
- [ ] Define framework-neutral DnD and property-editor contracts.
- [ ] Define contribution registry and renderer contracts.
- [ ] Define accessibility metadata and generic message identifiers.
- [ ] Add state-machine and policy tests independent from a browser.

**Phase gate:** a mock framework adapter can drive full and inline editing state solely through
`fly-ui` contracts.

### Phase 3 — Generic `fly-leptos`

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly-leptos` depending only on `fly-ui`, `fly` and generic Leptos crates.
- [ ] Implement canvas, layers, palette and panel components.
- [ ] Implement selection, DnD, resize, keyboard and inline overlays.
- [ ] Implement Leptos renderer and property-editor factories.
- [ ] Implement full, inline, preview and read-only presentations.
- [ ] Implement missing-plugin placeholders.
- [ ] Add a standalone example outside RusTok.
- [ ] Add accessibility and browser interaction tests.

**Phase gate:** a clean Leptos application embeds full and inline Fly editors without RusTok.

### Phase 4 — Backend Fly integration

- [ ] **Phase status:** not started.
- [ ] Add a dependency from `rustok-page-builder` backend to `fly` only.
- [ ] Replace synthetic tree inspection with Fly traversal.
- [ ] Route validation through Fly while preserving typed errors.
- [ ] Connect preview/rendering behind existing adapter seams.
- [ ] Preserve public capability names and envelopes.
- [ ] Keep sanitization and authorization authoritative in backend.
- [ ] Add runtime tests using Next GrapesJS projects.

**Phase gate:** the current Next editor saves a project that backend validates, previews and
publishes through Fly without a public contract break.

### Phase 5 — Page Builder admin surface

- [ ] **Phase status:** not started.
- [ ] Create `crates/rustok-page-builder/admin` with standard FFA structure.
- [ ] Implement full authoring shell over `fly-leptos`.
- [ ] Integrate RusTok admin theme, locale, permissions and degraded states.
- [ ] Implement admin native/server and GraphQL adapters.
- [ ] Implement typed save/preview/publish intents without consumer persistence.
- [ ] Implement `PageBuilderControl` for registry, policies, presets, migration and health.
- [ ] Update module manifest UI classification and admin wiring.
- [ ] Add manifest, FFA and headless parity verification.

**Phase gate:** admin authoring and control UI work in embedded and headless-compatible profiles
and disappear when the admin Page Builder surface is not composed.

### Phase 6 — Page Builder storefront surface

- [ ] **Phase status:** not started.
- [ ] Create `crates/rustok-page-builder/storefront` with standard FFA structure.
- [ ] Implement authenticated edit-mode activation.
- [ ] Implement inline overlays, toolbar, insertion controls and draft preview.
- [ ] Integrate storefront theme, locale, permissions and read-only states.
- [ ] Implement GraphQL and optional native/server adapters.
- [ ] Ensure anonymous/non-editor bundles can exclude editing code.
- [ ] Update module manifest storefront wiring.
- [ ] Add multi-deployment configuration tests.

**Phase gate:** the same storefront crate supports at least two independently configured
frontend deployments, and edit mode is absent when not enabled.

### Phase 7 — Generated contribution registries

- [ ] **Phase status:** not started.
- [ ] Define stable admin and storefront contribution factory signatures.
- [ ] Extend module manifests with surface-specific builder metadata.
- [ ] Generate admin registry from enabled admin contributions.
- [ ] Generate storefront registry from enabled storefront contributions.
- [ ] Pass registries through host composition without hard-coded optional dependencies.
- [ ] Filter by tenant state, permissions, policies and capabilities.
- [ ] Add duplicate ID, dependency-cycle and missing-provider diagnostics.

**Phase gate:** each surface composes only available contributions and disabling a provider does
not delete project nodes.

### Phase 8 — Pages pilot across both surfaces

- [ ] **Phase status:** not started.
- [ ] Add Pages admin blocks, traits, editor renderers and translations.
- [ ] Mount the Page Builder admin editor in Pages admin.
- [ ] Add Pages storefront published renderers and optional inline editing.
- [ ] Keep page metadata and lifecycle ownership in `rustok-pages`.
- [ ] Keep admin and storefront transports behind Pages-owned FFA facades.
- [ ] Preserve the JSON debug/fallback surface during rollout.
- [ ] Test builds without Page Builder, admin-only and full profiles.
- [ ] Add Next GrapesJS <-> Leptos Fly round-trip tests.
- [ ] Add `all_on`, `publish_off`, `preview_off` and `builder_off` tests.

**Phase gate:** one Page is alternately editable through Next GrapesJS, Fly admin and Fly
storefront edit mode without data loss or lifecycle regression.

### Phase 9 — Module contribution rollout

- [ ] **Phase status:** not started.

#### Wave A — platform foundations

- [ ] Media asset providers and media renderers.
- [ ] Pages reusable sections and menus.
- [ ] Surface-specific registry and i18n verification.

#### Wave B — primary content and commerce entities

- [ ] Blog admin/storefront contributions.
- [ ] Forum admin/storefront contributions.
- [ ] Product admin/storefront contributions.
- [ ] Pricing admin/storefront contributions.
- [ ] Taxonomy selector contribution after manifest-backed ownership.
- [ ] Owner-module SEO inspector contributions.

#### Wave C — composite and discovery widgets

- [ ] Commerce admin/storefront contributions.
- [ ] Search admin/storefront contributions.
- [ ] Comments contribution after published renderer contract exists.
- [ ] Profiles contribution after manifest-backed UI ownership exists.

For every contributing module:

- [ ] IDs and schema versions are documented;
- [ ] admin and storefront translations remain in their owning UI packages;
- [ ] shared widget configuration is framework-neutral;
- [ ] editor, inline and published renderers have clear ownership;
- [ ] native and GraphQL adapters have parity tests;
- [ ] missing-provider preservation is tested;
- [ ] behaviour without each Page Builder surface is documented;
- [ ] local docs and plans are updated;
- [ ] security, cache and invalidation ownership are recorded.

**Phase gate:** domain modules can be enabled independently on admin and storefront surfaces
without destructive project changes.

### Phase 10 — Published rendering and rollout completion

- [ ] **Phase status:** not started.
- [ ] Stabilize shared versioned widget configuration contracts.
- [ ] Complete module-owned storefront renderers.
- [ ] Define safe derived HTML/CSS caching without replacing project data.
- [ ] Define cache keys and invalidation events for dynamic widgets.
- [ ] Correlate editor save -> builder publish -> owner lifecycle -> storefront read.
- [ ] Replace synthetic rollout evidence with observed tenant packets.
- [ ] Complete rollback and legacy-block bridge exit criteria.

**Phase gate:** admin preview, storefront edit mode and published output have verified parity for
pilot modules and tenant rollout can be promoted or rolled back safely.

### Phase 11 — Additional framework adapters

- [ ] **Phase status:** deferred.
- [ ] Create `fly-dioxus` after `fly-ui` contracts stabilize.
- [ ] Reuse Fly state, UI contracts and module configuration schemas.
- [ ] Add framework-specific factories without domain duplication.
- [ ] Verify a complex module contribution across Leptos and Dioxus.

**Phase gate:** both framework adapters consume the same `fly-ui` state and project data.

### Phase 12 — Optional Fly repository extraction

- [ ] **Phase status:** deferred.
- [ ] Confirm `fly`, `fly-ui` and `fly-leptos` have no RusTok dependency leakage.
- [ ] Stabilize public APIs and semantic versioning.
- [ ] Decide license and publication policy.
- [ ] Extract Fly crates only when an independent release cadence is valuable.
- [ ] Keep Page Builder admin and storefront packages in RusTok.

**Phase gate:** extraction changes only dependency and release wiring, not Page Builder surface
ownership or consumer module architecture.

## Verification programme

Expected commands as packages are introduced:

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
```

Required suites cover:

- GrapesJS -> Fly -> GrapesJS round trips;
- Node reload through `loadProjectData()`;
- unknown field and plugin preservation;
- duplicate IDs and plugin dependency validation;
- widget schema migration;
- undo/redo and `fly-ui` state transitions;
- Leptos browser interaction;
- admin and storefront build-time composition;
- admin-only, storefront-only, full and no-builder profiles;
- multiple separately configured storefront deployments;
- runtime tenant filtering;
- native/server and GraphQL FFA parity per surface;
- admin editor, inline editor and published renderer parity;
- missing-provider fallback;
- sanitization and script rejection;
- asset permission boundaries;
- i18n completeness per contribution surface;
- accessibility and keyboard operation;
- project-size and history limits;
- publish idempotency, rollback and cache invalidation.

## Update rules

- This document is the central cross-module Fly programme plan.
- `crates/rustok-page-builder/docs/implementation-plan.md` remains the backend-provider plan
  and must link to this programme when implementation starts.
- Every contributing module updates its own implementation plan before a rollout task is marked
  complete here.
- Contract changes require matching verification changes in the same iteration.
- Checkboxes are updated only from merged code and reproducible evidence.
- After each phase, remove outdated wording that presents `fly-ui` as a RusTok bridge, treats
  admin and storefront as one deployment package, or places editor ownership in host code.
