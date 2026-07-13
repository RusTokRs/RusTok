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

The architecture has one explicit UI owner:

- `fly` is a standalone, framework-neutral Rust editor engine and GrapesJS-compatible
  project runtime;
- `fly-leptos` is a generic Leptos rendering and interaction adapter that can be used by
  any Leptos application;
- **Fly UI is the module-owned UI of `rustok-page-builder`**, implemented in
  `crates/rustok-page-builder/admin`;
- there is no separate top-level `crates/fly-ui` bridge;
- `rustok-page-builder/admin` owns the visual editor shell, canvas composition,
  drag-and-drop experience, generic builder chrome, contribution registry integration
  and the Page Builder control surface;
- module-owned UI packages contribute their own blocks, widgets, traits, settings,
  translations, editor renderers and published renderers;
- consumer module UIs such as Pages, Blog and Forum own document loading, saving,
  lifecycle and their own FFA transports, but they do not own or reimplement the visual
  editor;
- `rustok-page-builder` remains the backend FBA provider for validation, sanitization,
  preview, publish, rollout, permissions, persistence and rendering seams;
- no FFI crate is planned until a real non-Rust consumer requires one.

The visual editor starts with the Page Builder module, not with a host application. Hosts
only mount module-owned surfaces and compose generated contribution factories. If the
Page Builder module is absent from a build, the editor shell, palette, drag-and-drop and
builder management UI are absent as well; domain modules keep their non-builder CRUD and
fallback paths.

`fly` and `fly-leptos` may remain top-level workspace crates while they are developed,
because they must remain independently reusable and publishable. Their only RusTok-owned
product UI is `crates/rustok-page-builder/admin`.

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
- [x] The programme plan records that Fly UI belongs to `rustok-page-builder/admin`.
- [ ] Real GrapesJS fixtures and a compatibility matrix are committed.
- [ ] The `fly` and `fly-leptos` crates exist.
- [ ] `rustok-page-builder/admin` exists.
- [ ] A generated module contribution registry exists.

## Target architecture

```text
                                  FRONTEND

  consumer module UI
  Pages / Blog / Forum / Product / Commerce
       |
       | owns document load/save/lifecycle and its own FFA transport
       |
       +---- mounts -----------------------------------------------+
                                                                 |
                                                                 v
                                            rustok-page-builder/admin
                                            Fly UI owner
                                            +-- PageBuilderEditor
                                            +-- PageBuilderControl
                                            +-- RusTok theme/i18n/policy
                                            +-- contribution composition
                                                      |
                                                      v
                                                 fly-leptos
                                          generic Leptos adapter
                                                      |
                                                      v
                                                     fly
                                      engine, registries, commands, codec

  enabled module UI packages
       +-- pages builder contribution --------------------+
       +-- blog builder contribution ---------------------+--> generated registry
       +-- forum builder contribution --------------------+         |
       +-- media asset contribution ----------------------+         v
                                                     PageBuilderEditor

                                  BACKEND

  rustok-page-builder
       +-- validation / sanitization / preview / publish
       +-- RBAC / tenant scope / rollout / health
       +-- persistence and rendering ports
       +-- may depend on `fly`
       +-- must not depend on `fly-leptos` or its admin UI
```

The host application remains a technical composition root only. It may receive generated
wiring that imports enabled module factories and passes them to the Page Builder UI, but it
must not own editor behaviour, domain widgets, translations or persistence semantics.

## Physical package layout

```text
crates/
  fly/                              # standalone framework-neutral engine
  fly-leptos/                       # standalone generic Leptos adapter

  rustok-page-builder/
    src/                            # backend FBA provider
    admin/                          # Fly UI: module-owned Page Builder frontend
      locales/
        en.json
        ru.json
      src/
        core/
        model.rs
        contribution.rs
        transport/
          mod.rs
          graphql_adapter.rs
          native_server_adapter.rs
        editor/
          mod.rs
          page_builder_editor.rs
          registry.rs
          policies.rs
        control/
          mod.rs
          page_builder_control.rs
        ui/
          leptos/

  rustok-pages/
    admin/
      src/
        builder/                    # Pages widget contribution
        transport/                  # Pages FFA transport
        ui/                         # mounts PageBuilderEditor
    storefront/
      src/builder/                  # Pages published renderers

  rustok-forum/
    admin/src/builder/              # Forum widgets, traits, translations, preview UI
    storefront/src/builder/         # Forum published renderers
```

A separate module `builder-ui` sub-crate is not required initially. Contributions should
remain inside the existing module-owned admin/storefront UI packages unless a real compile
cycle or independently reusable contract justifies extraction.

## Dependency rules

```text
fly-leptos -> fly
rustok-page-builder-admin -> fly-leptos -> fly
consumer module admin UI -> rustok-page-builder-admin
module builder contribution -> fly and framework contribution contracts
rustok-page-builder backend -> fly
```

The generated contribution registry may depend on the enabled module UI packages and pass
factories into `rustok-page-builder-admin`. `rustok-page-builder-admin` itself must not
hard-code or directly depend on every optional domain UI package.

Forbidden dependencies:

```text
fly -X-> rustok-*
fly-leptos -X-> rustok-*
rustok-page-builder backend -X-> fly-leptos
rustok-page-builder backend -X-> rustok-page-builder-admin
rustok-page-builder-admin -X-> rustok-pages-admin / rustok-blog-admin /
                              rustok-forum-admin / other optional module UIs
module contribution -X-> host application code
```

This avoids the cycle:

```text
consumer UI -> Page Builder UI -> consumer UI
```

Instead, consumer UI and Page Builder UI meet through Fly contribution contracts and
generated composition wiring.

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
| component views | framework renderers in `fly-leptos` and later `fly-dioxus` |

Fly should preserve the same capabilities, but its public API must be idiomatic Rust rather
than a literal translation of the JavaScript API.

### Format reference

The first canonical project format remains `grapesjs_v1`. Test fixtures must be captured
from the real Next GrapesJS editor by calling `getProjectData()`. Hand-written fixture JSON
is not sufficient as the only compatibility evidence.

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

## Fly ecosystem layers

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

`fly` must not contain RusTok domain widgets such as forum topics, blog posts, product cards
or cart summaries.

### `fly-leptos`

`fly-leptos` is a generic Leptos adapter. It owns reusable framework mechanics:

- canvas rendering;
- generic block palette primitives;
- layers tree;
- trait and property panel primitives;
- style controls;
- drag and drop;
- selection overlays and resize handles;
- keyboard interaction;
- framework renderer registration;
- editor, preview and published render modes;
- missing-plugin placeholders.

A clean Leptos application must be able to use `fly` and `fly-leptos` without any RusTok
dependency. This does not make `fly-leptos` the RusTok Page Builder UI; it is the generic
framework toolkit used by that UI.

### Fly UI: `rustok-page-builder/admin`

Fly UI is the actual UI of the Page Builder module. It owns:

- the public reusable `PageBuilderEditor` component;
- the public `PageBuilderControl` management surface;
- complete RusTok editor shell and layout;
- block palette composition;
- canvas, layers, trait, style and asset panel composition;
- toolbar, viewport controls and drag-and-drop product behaviour;
- RusTok design-system bindings;
- host locale and i18n resolver bridging;
- host theme integration;
- permission, read-only and degraded-state mapping;
- standard RusTok errors, banners, dialogs and notifications;
- consumption of the generated contribution registry;
- build-time and tenant-time contribution filtering;
- conversion between generic Fly events and consumer-owned frontend intents;
- generic Page Builder chrome translations;
- registry, compatibility, preset, health and rollout management UI.

Fly UI must not:

- own Pages, Blog, Forum or Commerce document persistence;
- own domain widget translations;
- contain widgets from optional modules directly;
- call consumer module GraphQL or server functions;
- become the source of truth for tenant or RBAC policy;
- save or publish a consumer document without the consumer module lifecycle;
- expose contributions from modules absent from the build.

### Later `fly-dioxus`

`fly-dioxus` is deferred until the Fly engine and plugin API are stable. It must reuse the
same `fly` model and plugin metadata without duplicating domain logic.

## Public Fly UI surfaces

### `PageBuilderEditor`

The editor is mounted by a consumer module UI and receives:

- project data or a `FlyEditor` handle;
- the effective contribution registry;
- locale, theme and host context;
- permission and capability state;
- document-specific editor policy;
- callbacks or typed intents for project changes, preview, save requests and publish
  requests.

The editor emits intents; it does not decide the persistence lifecycle.

Illustrative flow:

```text
rustok-pages/admin
  -> loads Page through Pages transport facade
  -> creates editor input
  -> mounts PageBuilderEditor
  -> receives project-change intent
  -> saves through Pages transport facade
```

### `PageBuilderControl`

The control surface is a module-owned administrative UI. Through the Page Builder module's
own FFA adapters it manages:

- installed Fly core and module contribution inventory;
- build-time availability versus tenant runtime enablement;
- plugin compatibility and missing-provider diagnostics;
- global and tenant-level core-block allowlists;
- module-widget visibility policy within the enabled set;
- default presets and starter templates;
- project compatibility and migration diagnostics;
- provider health, degraded mode and rollout state;
- effective capability and permission information;
- links to module-owned widget settings where ownership belongs elsewhere.

It must not edit Pages, Blog, Forum, Product or Commerce documents directly.

When implemented, `rustok-page-builder/rustok-module.toml` must change from
`capability_only` to the correct UI classification and declare the admin UI package.

## Extension and plugin model

Fly owns the extension mechanism, not the complete catalogue of business widgets.

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

Every custom component identifier must be namespaced and stable:

```text
rustok.forum.latest_topics
rustok.blog.featured_post
rustok.product.product_grid
```

Each stored custom component must carry enough information to diagnose and migrate it:

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

Provider-owned migrations must be explicit and versioned. Fly may orchestrate a migration
but must not invent domain migrations for a module.

If the provider plugin is unavailable, Fly must:

- retain the complete node and unknown fields;
- render a missing-plugin placeholder in the editor;
- report the component type and required provider;
- prevent destructive implicit conversion;
- allow safe deletion only as an explicit user action.

## Module-owned widget contract

Module UI packages own their builder widgets. The Page Builder UI owns the editor but does
not centrally store all RusTok features.

A module contribution may include these roles:

- `document_consumer` — owns a document/template lifecycle and mounts
  `PageBuilderEditor` when the Page Builder module is available;
- `block_provider` — contributes palette blocks and component types;
- `trait_provider` — contributes selectors and custom property editors;
- `asset_provider` — contributes asset browsing or selection;
- `editor_renderer` — renders a widget inside the editing canvas;
- `storefront_renderer` — renders the published widget;
- `control_contribution` — contributes diagnostics or policy UI without a visual block.

Only `rustok-page-builder/admin` is the `editor_owner`. Consumer modules are not editor
hosts in the ownership sense; they mount the editor for documents they own.

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

`rustok-page-builder/admin` owns translations only for generic editor chrome and Page
Builder integration/control states. A Forum widget must never require a Forum string in
Fly, `fly-leptos` or the Page Builder UI package.

## FFA boundary for every module UI

Each module-owned UI is responsible for its own FFA boundary. The visual widget is shared
while transport is selected by the module frontend adapter.

```text
module widget UI
  -> module transport facade
     -> native/server adapter for embedded SSR/monolith
     -> GraphQL adapter for standalone/headless
  -> module backend service
```

Rules:

- UI components do not branch directly on GraphQL versus server functions;
- `fly`, `fly-leptos` and Page Builder UI never select consumer domain transports;
- both transports return transport-neutral module-owned types;
- native and GraphQL paths apply identical domain and authorization semantics;
- locale, tenant and auth context come from the host contract;
- widget preview may fetch live data only through its module transport facade;
- dynamic widgets store query/configuration traits, not resolved domain snapshots;
- the same widget UI works in embedded and headless-compatible profiles.

The Page Builder control surface has its own separate FFA facade because it talks to the
`rustok-page-builder` backend. That facade is not reused for Pages, Blog or Forum data.

## Composition and enablement

There are two distinct enablement boundaries.

### Build-time composition

`modules.toml` determines which module code and UI contributions are available in the
build. Contributions from absent modules must not enter the binary or frontend bundle.

If `page_builder` is absent:

- `rustok-page-builder/admin` is not mounted;
- `fly-leptos` is not pulled into RusTok through the Page Builder feature path;
- no Page Builder palette, canvas or drag-and-drop UI is available;
- consumer modules retain normal CRUD, read and fallback surfaces;
- existing builder project data is not deleted or rewritten merely because the editor is
  unavailable.

### Runtime tenant composition

Within the build-time set, tenant module enablement and builder policy determine which
contributions are visible and usable.

The generated registry composes only build-available factories. Fly UI then filters the
available set by tenant module state, permissions, block allowlists and builder capability
state.

A project containing a component from a disabled or unavailable module remains lossless
and displays a diagnostic placeholder. Runtime disablement must not delete project nodes.

## Contribution registry and host wiring

Each contributing module UI exports a stable factory, for example:

```rust
pub fn builder_contribution() -> ModuleBuilderContribution;
```

Manifest metadata identifies the factory and supported roles. Build/code generation creates
a registry from only enabled modules. The registry is supplied to Page Builder UI through
host/module composition context.

The host is allowed to perform only mechanical wiring:

```text
enabled module factories
  -> generated registry
  -> PageBuilderUiContext
  -> PageBuilderEditor
```

The host must not:

- define widget schemas;
- translate module widgets;
- select widget transports;
- implement editor commands;
- own builder policy;
- save consumer documents.

## Module UI support matrix

The matrix defines planned initial ownership. It is not permission to move the listed
widgets into Fly core or Page Builder UI.

| Module UI | Planned role | Initial contribution scope | Delivery wave |
|---|---|---|---|
| `rustok-page-builder/admin` | `editor_owner`, `control_surface`, core `block_provider` | full Fly UI, generic blocks, registry, policies, presets, compatibility, health and rollout management | Fly UI phase |
| `rustok-pages/admin` | `document_consumer`, `block_provider`, `trait_provider` | full page editing, page links, menus and reusable sections | Pilot |
| `rustok-pages/storefront` | `storefront_renderer` | published page layouts and Pages-owned widgets | Pilot publish |
| `rustok-media/admin` | `asset_provider`, `block_provider` | media picker, image, gallery and video asset integration | Wave A |
| `rustok-blog/admin` | `document_consumer`, `block_provider`, `trait_provider` | blog templates, latest posts, featured post, category feed and author card | Wave B |
| `rustok-blog/storefront` | `storefront_renderer` | published blog widgets and templates | Wave B |
| `rustok-forum/admin` | `document_consumer`, `block_provider`, `trait_provider` | forum templates, latest topics, popular discussions, category list and topic feed | Wave B |
| `rustok-forum/storefront` | `storefront_renderer` | published forum widgets and templates | Wave B |
| `rustok-product/admin` | `block_provider`, `trait_provider`, optional `document_consumer` | product card, product grid, recommendations, category carousel and product templates | Wave B |
| `rustok-product/storefront` | `storefront_renderer` | published product widgets and templates | Wave B |
| `rustok-pricing/admin` | `trait_provider`, `block_provider` | price display configuration and pricing-table widgets | Wave B |
| `rustok-pricing/storefront` | `storefront_renderer` | published price and pricing-table rendering | Wave B |
| `rustok-taxonomy` UI contribution | `trait_provider`, optional `block_provider` | taxonomy selectors, category navigation and query configuration | Wave B after a manifest-backed UI owner exists |
| owner SEO panels and `rustok-seo/admin` support | `control_contribution` | SEO inspector and metadata settings; no generic visual block ownership by default | Wave B |
| `rustok-commerce/admin` | `document_consumer`, `block_provider` | merchandising layouts, cart summary, checkout CTA and commerce composition | Wave C |
| `rustok-commerce/storefront` | `storefront_renderer` | published commerce widgets | Wave C |
| `rustok-search/admin` | `block_provider`, `trait_provider` | search box, result list and facet configuration | Wave C |
| `rustok-search/storefront` | `storefront_renderer` | published search widgets | Wave C |
| `rustok-comments/admin` | future `block_provider` | comment feed, count and discussion embedding after a published renderer contract exists | Wave C |
| `rustok-profiles` UI contribution | future `block_provider` | member, author and profile widgets after a manifest-backed UI package exists | Wave C |
| `rustok-region` UI packages | `trait_provider` where required | region and availability selectors, not a default block catalogue | As needed |
| `rustok-channel/admin` | `trait_provider` where required | channel targeting and visibility controls | As needed |
| `rustok-workflow/admin` | `control_contribution` | publish workflow and status controls, not domain visual blocks by default | As needed |

Modules not listed as initial providers, including Order, Payment, Fulfillment, Inventory
and Customer, must not expose privileged operational actions as visual page blocks by
default. Any later contribution requires its own security review, UI ownership,
published-renderer contract and FFA parity tests.

## Runtime flows

### Editing a document

```text
consumer module UI loads project through its own transport facade
  -> creates document editor input
  -> mounts rustok-page-builder/admin::PageBuilderEditor
  -> Fly UI applies RusTok context, policies and generated contributions
  -> fly-leptos renders generic interaction primitives
  -> Fly emits project-change and UI-intent events
  -> consumer module UI decides when and how to save
```

Fly UI does not save a Page, Post, Forum template or Product template by itself.

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
  -> consumer module transport facade
  -> rustok-page-builder capability endpoint where validation/publish is required
  -> owner module persistence and lifecycle service
  -> module-owned storefront renderer resolves published widgets
```

The backend remains authoritative for permissions, sanitization, tenant scope,
idempotency and publish state.

### Missing or disabled Page Builder module

```text
consumer module UI starts without page_builder
  -> no PageBuilderEditor mount is available
  -> visual palette/canvas/drag-and-drop are absent
  -> consumer uses its documented fallback or read-only path
  -> canonical project data remains untouched
```

### Missing widget provider

```text
project contains rustok.forum.latest_topics
  -> Forum contribution unavailable
  -> Fly retains the raw node
  -> editor shows a missing-provider placeholder when the editor is available
  -> save remains lossless
  -> deletion is allowed only by explicit user action
```

## Project and widget versioning

- `grapesjs_v1` remains the outer project contract during the compatibility period.
- Fly crate versions and project-contract versions are separate concerns.
- Core Fly block identifiers become stable after the first compatibility release.
- Each module widget has a stable namespaced type and independent schema version.
- Provider modules own widget migrations.
- Migration is explicit, observable and reversible where possible.
- A project records diagnostics when a widget schema is newer than the installed provider
  understands.
- Generated HTML/CSS may be cached as a derived publish artifact but must not replace
  canonical project data.

## Security and operational requirements

- arbitrary component scripts are disabled by default;
- raw HTML, URLs, attributes and CSS pass backend sanitization policy;
- editor preview is not trusted published output;
- dynamic widgets cannot bypass module RBAC through frontend preview calls;
- asset selection integrates with module-owned media permissions;
- tenant context is mandatory for widget data access;
- publish writes require deadline and idempotency semantics;
- missing or disabled plugins never cause silent data deletion;
- disabling Page Builder never causes silent project conversion or deletion;
- widget dependency cycles are rejected by registry validation;
- plugin and renderer panics/errors become typed diagnostics;
- published dynamic widgets define cache keys and invalidation ownership;
- editor and storefront renderers have parity tests for significant states;
- accessibility and keyboard interaction are part of the editor acceptance gate;
- project-size and history limits are configurable to prevent browser memory exhaustion.

## Implementation phases

### Phase 0 — Baseline, ADR and compatibility evidence

- [ ] **Phase status:** in progress.
- [x] Keep the current Next GrapesJS editor operational as the reference editor.
- [x] Keep `grapesjs_v1` as the current backend and consumer contract.
- [x] Preserve the existing `rustok-page-builder` FBA provider boundary.
- [x] Preserve the Pages JSON editor as a fallback.
- [x] Record in this programme that Fly UI is owned by `rustok-page-builder/admin`.
- [ ] Add an ADR recording the final ownership and dependency decisions.
- [ ] Capture real GrapesJS fixtures for basic page, multi-page, styles/selectors,
  assets, traits, custom components and plugin metadata.
- [ ] Record the GrapesJS version and enabled plugins for every fixture.
- [ ] Add a Node compatibility harness that reloads Fly-produced fixtures through
  `loadProjectData()`.
- [ ] Record the support matrix in module-local plans before each module implementation.
- [ ] Add source guards rejecting RusTok dependencies from `fly` and `fly-leptos`.

**Phase gate:** fixtures and compatibility expectations are reproducible in CI, ownership
is captured by ADR, and the reference Next editor remains unchanged in behaviour.

### Phase 1 — `fly` lossless project model and codec

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly` with README, docs and implementation plan.
- [ ] Implement a raw lossless project representation.
- [ ] Implement typed accessors for pages, frames, components, styles, selectors, assets
  and traits.
- [ ] Preserve unknown fields at every extensible level.
- [ ] Implement deterministic serialization without semantic normalization.
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
- [ ] Implement the initial generic block set.
- [ ] Implement plugin dependency and duplicate-ID validation.
- [ ] Add tests constructing and mutating projects only through the Fly API.

**Phase gate:** a non-visual Rust test can construct, edit, undo, redo, serialize and
reload a project containing core and custom plugin components.

### Phase 3 — Backend Fly integration

- [ ] **Phase status:** not started.
- [ ] Add a dependency from the `rustok-page-builder` backend to `fly` only.
- [ ] Replace synthetic tree inspection with Fly traversal.
- [ ] Route project validation through Fly while preserving the typed error catalogue.
- [ ] Connect Fly-backed preview/rendering behind the existing rendering adapter seam.
- [ ] Preserve public FBA request/response envelopes and capability names.
- [ ] Keep sanitization and authorization authoritative in the backend module.
- [ ] Add runtime tests using projects saved by the Next GrapesJS editor.

**Phase gate:** the current Next editor can save a project that the backend validates,
inspects, previews and publishes through Fly without a public contract break.

### Phase 4 — Generic `fly-leptos`

- [ ] **Phase status:** not started.
- [ ] Create `crates/fly-leptos` with no RusTok dependencies.
- [ ] Implement canvas, layers and block palette primitives.
- [ ] Implement selection, drag/drop, resize and keyboard interaction.
- [ ] Implement generic trait, style and asset panel primitives.
- [ ] Implement framework renderer registration for plugin components.
- [ ] Implement editor, preview and published render modes.
- [ ] Implement the missing-plugin placeholder.
- [ ] Add an independent example application outside RusTok module UI packages.
- [ ] Add accessibility and browser interaction tests.

**Phase gate:** a clean Leptos application can build an editor using only `fly` and
`fly-leptos`, without importing RusTok.

### Phase 5 — Fly UI in `rustok-page-builder/admin`

- [ ] **Phase status:** not started.
- [ ] Create `crates/rustok-page-builder/admin` with module-standard
  `core/model/transport/ui` FFA structure.
- [ ] Implement the public `PageBuilderEditor` component.
- [ ] Compose generic Fly/Leptos primitives into the complete RusTok editor shell.
- [ ] Integrate RusTok UI primitives, theme, host context and i18n.
- [ ] Map permissions, read-only mode and degraded capabilities into editor state.
- [ ] Implement generic editor chrome translations in the Page Builder UI package.
- [ ] Export typed project-change, preview, save-request and publish-request intents.
- [ ] Verify the editor performs no consumer module persistence or transport calls.
- [ ] Update `rustok-page-builder/rustok-module.toml` to declare the admin UI package and
  correct UI classification.
- [ ] Add manifest and module UI package verification.

**Phase gate:** the Page Builder module owns and exports a working Fly UI editor. Removing
the module from composition removes its editor, palette and drag-and-drop surfaces without
breaking consumer CRUD.

### Phase 6 — Contribution registry and Page Builder control UI

- [ ] **Phase status:** not started.
- [ ] Define the module UI contribution contract and stable factory signature.
- [ ] Extend module manifest metadata for builder contributions.
- [ ] Generate the build-time registry from enabled modules.
- [ ] Pass the registry into Fly UI without direct optional-module dependencies.
- [ ] Filter build-available contributions by tenant runtime state and permissions.
- [ ] Implement the public `PageBuilderControl` surface.
- [ ] Add Page Builder native/server and GraphQL adapters with semantic parity.
- [ ] Expose registry inventory, compatibility diagnostics and provider health.
- [ ] Expose core-block allowlists and module-widget visibility policy.
- [ ] Expose presets and starter-template management.
- [ ] Expose migration diagnostics and explicit migration actions.
- [ ] Show effective rollout, degraded-state and permission information.
- [ ] Add FFA and headless parity verification for the control surface.

**Phase gate:** Fly UI composes only enabled contributions, and the same Page Builder
control UI works in embedded and headless-compatible profiles through backend policy and
RBAC.

### Phase 7 — Pages visual-editor pilot

- [ ] **Phase status:** not started.
- [ ] Add a Pages-owned builder contribution inside `rustok-pages/admin`.
- [ ] Add Pages blocks, traits, editor renderers and translations.
- [ ] Mount `rustok-page-builder-admin::PageBuilderEditor` in Pages admin.
- [ ] Keep the JSON textarea as an explicit debug/fallback surface during rollout.
- [ ] Keep page metadata and lifecycle ownership in `rustok-pages`.
- [ ] Keep native/server and GraphQL paths behind the Pages transport facade.
- [ ] Ensure Pages admin works without the Page Builder feature using its documented
  fallback path.
- [ ] Add Next GrapesJS <-> Leptos Fly cross-editor round-trip tests.
- [ ] Add tenant fallback tests for `all_on`, `publish_off`, `preview_off` and
  `builder_off`.

**Phase gate:** the same Page can be opened, edited and saved alternately by Next
GrapesJS and Leptos Fly without loss, while a build without Page Builder retains Pages
CRUD without visual editing.

### Phase 8 — Module contribution rollout

- [ ] **Phase status:** not started.

#### Wave A — platform foundations

- [ ] `rustok-media/admin`: asset provider and media widgets.
- [ ] Pages-owned reusable sections and menus.
- [ ] Generated contribution and i18n completeness verification.

#### Wave B — primary content and commerce entities

- [ ] `rustok-blog` admin/storefront contributions and optional document editing.
- [ ] `rustok-forum` admin/storefront contributions and optional layout editing.
- [ ] `rustok-product` admin/storefront contributions.
- [ ] `rustok-pricing` admin/storefront contributions.
- [ ] Taxonomy selector contribution after a manifest-backed UI owner is established.
- [ ] Owner-module SEO inspector contributions.

#### Wave C — composite and discovery widgets

- [ ] `rustok-commerce` admin/storefront contributions.
- [ ] `rustok-search` admin/storefront contributions.
- [ ] Comments contribution after a published-renderer contract exists.
- [ ] Profiles contribution after a manifest-backed UI package exists.

For every contributing module:

- [ ] plugin IDs and schema versions are documented;
- [ ] translations live in the module UI package;
- [ ] editor renderer and published renderer are module-owned;
- [ ] native and GraphQL FFA adapters have parity tests;
- [ ] missing-provider preservation is tested;
- [ ] behaviour without the Page Builder module is documented and tested;
- [ ] local module docs and implementation plans are updated;
- [ ] security and cache ownership are recorded.

**Phase gate:** disabling a domain module removes its contributions without deleting
project nodes, and disabling Page Builder removes visual editing while preserving consumer
fallbacks and canonical project data.

### Phase 9 — Published rendering and rollout completion

- [ ] **Phase status:** not started.
- [ ] Define a shared versioned widget configuration contract between admin and storefront
  packages.
- [ ] Compile and cache safe derived HTML/CSS where useful without replacing project data.
- [ ] Complete module-owned storefront renderers.
- [ ] Define cache keys and invalidation events for dynamic widgets.
- [ ] Correlate editor save -> builder publish -> owner lifecycle -> storefront read.
- [ ] Replace synthetic Wave evidence with observed tenant packets.
- [ ] Complete rollback and legacy-block bridge exit criteria.

**Phase gate:** preview and storefront output have verified parity for pilot modules, and a
tenant rollout can be promoted or rolled back without redeploying.

### Phase 10 — Dioxus adapter

- [ ] **Phase status:** deferred.
- [ ] Create `fly-dioxus` only after Fly and plugin contracts are stable.
- [ ] Reuse module core models and contribution metadata.
- [ ] Add Dioxus-specific renderers without duplicating domain or transport semantics.
- [ ] Implement a Dioxus Page Builder UI adapter owned by the Page Builder module when a
  Dioxus admin host exists.
- [ ] Verify behaviour against the Leptos adapter.

**Phase gate:** at least one complex module contribution works through Leptos and Dioxus
with the same project data and domain behaviour.

### Phase 11 — Optional repository extraction

- [ ] **Phase status:** deferred.
- [ ] Confirm `fly` and `fly-leptos` have no RusTok dependency leakage.
- [ ] Stabilize public API and semantic versioning.
- [ ] Decide license and publication policy.
- [ ] Extract only `fly` and framework-generic adapters when an independent release cadence
  provides more value than monorepo development.
- [ ] Keep RusTok Fly UI in `rustok-page-builder/admin`.

**Phase gate:** extraction requires no Page Builder UI ownership change and no consumer
module refactor, only dependency and release wiring changes.

## Verification programme

Expected commands as crates and scripts are introduced:

```text
cargo test -p fly
cargo test -p fly-leptos
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
- absence of Fly UI and drag-and-drop when Page Builder is not composed;
- consumer CRUD/fallback behaviour without Page Builder;
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

The implementation explicitly accounts for:

- Fly UI as the module-owned UI of Page Builder rather than a generic top-level bridge;
- the Page Builder module as the only visual editor owner;
- consumer modules as document lifecycle owners and widget providers;
- removal of visual editing and drag-and-drop when Page Builder is absent;
- preservation of consumer CRUD and canonical project data without Page Builder;
- generated composition without direct optional-module dependencies;
- a frontend Page Builder control surface;
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
- After each phase, search for and remove outdated wording that presents the editor as
  host-owned, consumer-owned, GrapesJS-only or separate from the Page Builder UI module.
