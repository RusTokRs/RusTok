---
id: doc://docs/modules/manifest.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# `modules.toml` and `rustok-module.toml` Contract

This document describes two related layers of the RusToK module contract:

- `modules.toml` at the repository root defines the composition of platform modules for a specific build.
- `rustok-module.toml` inside a path module defines the publish/runtime/UI contract of the module itself.

`modules.toml` is responsible for the composition root. `rustok-module.toml` is responsible for identity, surface wiring, UI packages and publish-ready metadata.

## Where Each Contract Lives

### `modules.toml`

The root manifest captures:

- List of platform modules included in the build;
- Source of each module: `path`, `git`, `crates-io`/`registry`;
- Coarse-grained dependencies through `depends_on`;
- `runtime = "module" | "extension"`: `module` is the default and is
  materialized into `ModuleRegistry`; `extension` is a catalogued capability
  contribution that is composed through the generic runtime/transport extension
  seam and must not require an application-specific registry entry;
- Platform-level settings, including `settings.default_enabled`.

This is a runtime/build-level contract for the entire platform, not for an individual crate.

### `rustok-module.toml`

The local manifest of a path module captures:

- `module.slug`, `module.name`, `module.version`, `module.description`;
- `module.ui_classification`;
- `[crate].entry_type` for the runtime module;
- Runtime entry points of the module;
- Admin/storefront UI wiring;
- Module-owned settings schema;
- Marketplace/publish metadata;
- Dependencies and conflicts related to the module itself.

For path modules from `modules.toml`, the presence of `rustok-module.toml` is mandatory.

### `module.ui_classification`

`module.ui_classification` is mandatory for every path module and must match the actual UI wiring.

Supported values:

- `dual_surface`
- `admin_only`
- `storefront_only`
- `no_ui`
- `capability_only`
- `future_ui`

Practical rule for the current platform scope:

- A module with `[provides.admin_ui]` and `[provides.storefront_ui]` must have `dual_surface`;
- A module with only `[provides.admin_ui]` must have `admin_only`;
- A module with only `[provides.storefront_ui]` must have `storefront_only`;
- A module without UI may use `no_ui`, `capability_only` or `future_ui`, but must not simultaneously declare UI sub-crates.

### `[crate].entry_type`

If a crate implements `RusToKModule`, `rustok-module.toml` must contain `[crate].entry_type` matching the actual runtime entry type from `src/lib.rs`.

Practical rule:

- `pub struct BlogModule;` + `impl RusToKModule for BlogModule` require `entry_type = "BlogModule"`;
- A capability crate without `RusToKModule` may omit `entry_type`.

An `extension` entry in `modules.toml` is intentionally not a
`ModuleRegistry` entry. It may still own GraphQL/native transport and module
UI declarations in `rustok-module.toml`. The host receives it through the
generic contribution mechanism; adding a capability-specific import or feature
to `apps/server` is not a substitute for that mechanism.

### Synchronization of Runtime Metadata

If a crate implements `RusToKModule`, the values of `module.slug`, `module.name` and `module.description`
in `rustok-module.toml` must match `slug()`, `name()` and `description()` in `src/lib.rs`.

### `provides.graphql` and `provides.http`

If `rustok-module.toml` declares:

- `[provides.graphql].query`
- `[provides.graphql].mutation`
- `[provides.http].routes`
- `[provides.http].axum_router`
- `[provides.http].webhook_routes`

then the corresponding type/function symbols must actually exist inside `src/**/*.rs`
of the module. The manifest must not reference decorative or already removed transport surfaces.

`routes` is not a live manifest field and must not be declared by a live module.
`axum_router` is the active entrypoint: it names a function that accepts
`&rustok_api::HostRuntimeContext` and returns the module-owned `axum::Router`
(or a fallible result containing one). The host code generator mounts that
entrypoint directly; there is no parallel routing model.

### `provides.cli`

`[provides.cli]` is optional metadata for module-local operational command
adapters. It does not make the module domain crate depend on CLI parsing or
terminal concerns. The provider implementation must live in a module-local
`cli/` adapter package or external integration package, and it must implement
`rustok-cli-core` command/provider contracts.

Supported keys:

- `namespace`: optional command namespace; defaults to the module slug and must
  use `snake_case`.
- `factory`: Rust path to a function returning `Box<dyn CommandProvider>`.
- `provider`: Rust path to a provider type that implements `Default` and
  `CommandProvider`; the generated registry wraps it with `Box::new(...)`.

Declare exactly one of `factory` or `provider`. The selected distribution
registry is generated from this metadata into `rustok-cli-registry`; it is not
hand-maintained inside `rustok-cli`.

## Mandatory Minimum for a Path Module

Every path module from `modules.toml` must have:

- `Cargo.toml`;
- Root `README.md` in English;
- `docs/README.md` in English;
- `docs/implementation-plan.md` in English;
- `rustok-module.toml`.

The root `README.md` is considered part of the acceptance contract and must contain:

- `## Purpose`
- `## Responsibilities`
- `## Entry points`
- `## Interactions`
- A link to the local `docs/README.md`

Local docs are needed even for modules without admin/storefront UI.

### Minimum Documentation Contract

For a path module, the documentation contract is considered closed only if both layers are satisfied:

- Root `README.md` in English with sections `Purpose`, `Responsibilities`, `Entry points`, `Interactions` and a link to `docs/README.md`;
- Local `docs/README.md` in English as the live runtime/module contract;
- Local `docs/implementation-plan.md` in English as the live plan for bringing the module to its target state.

Minimum skeleton of local `docs/README.md`:

- `## Purpose`
- `## Responsibility Zone`
- `## Integration`
- `## Verification`
- `## Related Documents`

Minimum skeleton of local `docs/implementation-plan.md`:

- `## Scope`
- `## Current State`
- `## Milestones`
- `## Verification`
- `## Update Rules`

Additional sections are allowed, but this minimum must be preserved.

## What `cargo xtask module validate` Checks

`cargo xtask module validate <slug>` works only for slugs from `modules.toml` and validates the actual scoped contract:

`cargo xtask module validate` without slug iterates all local `source = "path"` modules from `modules.toml`. This is not auto-discovery by `crates/`: a new crate becomes a platform module only after being added to `[modules]`.

- Slug exists in `modules.toml`;
- For `source = "path"`, `path` is specified;
- `rustok-module.toml` exists at the expected path;
- `module.slug` matches the slug from `modules.toml`;
- `module.version` in `rustok-module.toml` matches the version from `Cargo.toml`;
- `module.ui_classification` exists, uses a supported value and is consistent with actual UI surfaces;
- If a crate implements `RusToKModule`, `[crate].entry_type` exists and matches the runtime entry type;
- If a crate implements `RusToKModule`, `module.slug`, `module.name` and `module.description` match the runtime metadata in `src/lib.rs`;
- If a module is marked as `required = true` in `modules.toml`, the runtime type explicitly returns `ModuleKind::Core`; an optional module does not declare `ModuleKind::Core`;
- If a crate implements `RusToKModule`, its `permissions()` contains no duplicates, uses only existing `Permission::*` constants or valid `Resource::*/Action::*` pairs from `rustok-core`, and covers the minimum runtime RBAC surface where this minimum is already fixed by platform contracts;
- For modules whose event-driven behavior has already been moved to a module-owned runtime path (`index`, `search`, `workflow`), `src/lib.rs` publishes listeners through `register_event_listeners(...)`, rather than falling back to hidden host-owned wiring;
- For `workflow`, webhook ingress remains a module-owned surface: the module holds `controllers::webhook_routes()`, and `apps/server` only re-exports it through a shim; the cron path is not mixed with webhook/event listener wiring;
- The `index != search` boundary remains a hard runtime contract: `index` publishes indexing/read-model substrate and module-owned listeners, while `search` publishes `SearchEngineKind`, `PgSearchEngine`, `SearchIngestionHandler`, `search_documents` and search UX/diagnostics surfaces, not mixing these layers in one module;
- `search` holds the operator-plane contract as part of the module surface: `SearchDiagnosticsService`, `SearchAnalyticsService`, `SearchSettingsService`, `SearchDictionaryService`, documented control-plane markers in `README.md` and local `docs/observability-runbook.md` are not considered optional noise and must not be lost during refactoring;
- If `[provides.graphql]` or `[provides.http]` are declared, the corresponding symbols actually exist in the module code; `provides.http.routes` and `provides.http.axum_router` are mutually exclusive;
- If a server shim exists for a module in `apps/server/src/controllers/<slug>/`, it exports `pub routes()` and/or `pub webhook_routes()` for all declared HTTP surfaces;
- `package.license` resolves through `Cargo.toml` or workspace inheritance;
- `module.description` is sufficient for publish readiness;
- `depends_on` from `modules.toml`, `[dependencies]` in `rustok-module.toml` and `RusToKModule::dependencies()` do not diverge;
- An optional module has a `mod-<slug>` feature in `apps/server/Cargo.toml`, this feature resolves to a real `ModuleRegistry` entry, and its `mod-*` dependencies match `depends_on` from `modules.toml`;
- For a `capability_only` ghost module, an always-linked server dependency path is acceptable: `mod-<slug>` may be an empty feature-guard for registry/codegen wiring if the crate itself is already connected to `apps/server` as a shared capability dependency;
- A `required = true` module is registered directly in `apps/server/src/modules/mod.rs`, and an optional module does not bypass feature/codegen wiring;
- `settings.default_enabled` lists only optional modules; required/core modules are not included there and are considered always active;
- `settings.default_enabled` forms a dependency-closed optional graph: if an optional module is enabled by default, its optional dependencies are also present in `default_enabled`;
- Each slug from `settings.default_enabled` is present in the default feature-set of the server as `mod-<slug>`;
- Root `README.md`, `docs/README.md` and `docs/implementation-plan.md` are present and match the minimum format;
- Wiring for `admin/` and `storefront/` is consistent with `[provides.admin_ui]` and `[provides.storefront_ui]`;
- If a UI sub-crate is declared in the manifest, its `Cargo.toml` actually exists and the version matches the main module version.
- `[provides.admin_ui]` requires not only `leptos_crate`, but also non-empty `route_segment`, `nav_label` and `[provides.admin_ui.i18n]` with `default_locale`, `supported_locales`, `leptos_locales_path`.
- `[provides.admin_ui].nav_group` and `nav_order` are optional navigation metadata for the host sidebar. If not specified, `apps/admin` uses standard groups `Content`, `Commerce`, `Runtime`, `Governance`, `Automation`, `Other`.
- `[[provides.admin_ui.child_pages]]` is canonical metadata for nested admin navigation: each item declares `subpath`, `title`, `nav_label` and is mounted under `/modules/:route_segment/:subpath`.
- The presence of `[settings]` in `rustok-module.toml` does not create a module-owned settings page. The host shows a contextual settings link at `/modules?module_slug=<slug>` and uses the existing tenant settings editor.
- `[provides.storefront_ui]` requires not only `leptos_crate`, but also non-empty `slot`, `route_segment`, `page_title` and `[provides.storefront_ui.i18n]` with `default_locale`, `supported_locales`, `leptos_locales_path`. The `slot` value must be one of the platform-known slots: `header_navigation`, `home_after_hero`, `home_after_catalog`, `home_before_footer`, `footer_navigation`, `checkout_shipping_handoff`, `checkout_payment_handoff`, `checkout_result_handoff`.
- `[[provides.storefront_ui.components]]` declares additional no-prop Leptos contributions from the same module UI crate. Each item requires a unique `id`, exported Rust `component`, platform-known `slot`, and may set deterministic `order`. The host registers these through the generic storefront registry; host source must not import module-specific layout components directly.
- If a UI sub-crate is declared in the manifest, the corresponding host (`apps/admin` or `apps/storefront`) actually connects it as a dependency and forwards mandatory host feature links (`/hydrate`, `/ssr`) where the sub-crate exports them.
- The host dependency on a UI sub-crate points to the canonical module path (`crates/<module>/admin` or `crates/<module>/storefront`), not to an arbitrary compatible crate with the same name.
- If a module publishes `admin_ui` or `storefront_ui`, the host composition includes UI surfaces of its direct module dependencies for the same surface when those dependencies also publish such UI.
- `apps/admin` and `apps/storefront` do not contain orphaned first-party UI dependencies: a path dependency on `crates/*/admin` or `crates/*/storefront` is allowed only if the corresponding `rustok-module.toml` actually declares this crate as `admin_ui` or `storefront_ui`.
- `apps/admin` and `apps/storefront` do not contain orphaned host feature entries: `hydrate`/`ssr` do not reference `crate/feature` for a first-party module UI crate if that crate is no longer declared in the module manifest or is no longer connected as a host dependency.
- Central navigation does not lag behind manifest wiring: `docs/modules/_index.md` contains docs/plan links of the module, and `docs/modules/UI_PACKAGES_INDEX.md` lists declared admin/storefront UI surfaces.

If a slug is missing from `modules.toml`, `xtask` returns `Unknown module slug`.

## What `cargo xtask validate-manifest` Checks

`cargo xtask validate-manifest` checks the central composition contract:

- `modules.toml` parses and uses a supported schema version;
- `default_enabled` references only actually declared modules;
- `does not contain missing slugs;
- `source` specification is valid for each module;
- `apps/server` holds the module-owned event runtime path through the shared `module_event_dispatcher`;
- All path modules actually contain `rustok-module.toml`.

This step does not replace `cargo xtask module validate <slug>`, but complements it.

Description of the workspace tool itself, its responsibilities and operator entrypoints lives in [`xtask/README.md`](../../xtask/README.md).

## Build-time Manifest Compiler Assessment

The current production baseline leaves `modules.toml`, `rustok-module.toml`, `cargo xtask validate-manifest` and scoped
`cargo xtask module validate <slug>` as the canonical manifest validation path. A separate build-time manifest compiler is not yet
introduced as a release blocker.

Reasons:

- Runtime composition already has two validation levels: general composition contract and scoped module contract;
- Production runtime stores immutable manifest snapshot/hash in `platform_state`, so the active module composition does not depend
  on ad-hoc auto-discovery by workspace;
- A compiler before extracting a shared foundation boundary will strengthen coupling between `apps/server`, `xtask` and registry/bootstrap
  validation, because it will start generating host wiring before ownership boundary stabilization;
- Current guardrails require explicit optional module wiring through `mod-<slug>` features and registry entry, which better
  corresponds to a controlled rollout.

Recommended order if coupling between `apps/server` and registry/bootstrap validation becomes a blocker again:

1. First extract a shared foundation contract for reading and normalizing `modules.toml`/`rustok-module.toml`.
2. Move shared DTOs, canonical hash and pure validation rules there without dependency on server runtime.
3. Leave `apps/server` as the owner of runtime wiring, and `xtask` as the owner of preflight/CI validation.
4. Only after that consider a build-time compiler as a thin generator over the foundation package.

The first step is partially done: `rustok-api::module_registry_contract` now owns framework-independent
comparison of manifest snapshot and runtime registry snapshot. Checks for missing runtime entry,
`required`/`core` mismatch and dependency mismatch are no longer implemented inside `apps/server`;
the composition root only transforms its runtime DTOs into the shared contract. File loading,
package-manifest overlay and actual `ModuleRegistry` wiring remain in `apps/server`.

Until such extraction, a build-time compiler is considered a deferred optimization, not a mandatory production gate.

## Minimal `modules.toml` Example

```toml
schema = 2
app = "rustok-server"

[modules]
blog = { crate = "rustok-blog", source = "path", path = "crates/rustok-blog", depends_on = ["content"] }
content = { crate = "rustok-content", source = "path", path = "crates/rustok-content" }

[settings]
default_enabled = ["content", "blog"]
```

## Minimal `rustok-module.toml` Example

```toml
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
description = "Blog module with admin and storefront surfaces."
ownership = "first_party"
trust_level = "verified"

[crate]
entry_type = "BlogModule"

[provides.graphql]
query = "graphql::BlogQuery"
mutation = "graphql::BlogMutation"

[provides.http]
routes = "controllers::routes"

[provides.cli]
namespace = "blog"
factory = "rustok_blog_cli::command_provider"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
route_segment = "blog"
nav_label = "Blog"
nav_group = "Content"
nav_order = 20

[[provides.admin_ui.child_pages]]
subpath = "posts"
title = "All Blog Posts"
nav_label = "All Posts"

[[provides.admin_ui.child_pages]]
subpath = "new"
title = "Add Blog Post"
nav_label = "Add Post"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
route_segment = "blog"
page_title = "Blog"
slot = "home_after_catalog"

[marketplace]
category = "content"
publisher = "rustok"
tags = ["blog", "editorial"]
description = "Blog module with admin and storefront surfaces."
```

## UI Sub-crate Invariants

- The presence of `admin/Cargo.toml` without `[provides.admin_ui].leptos_crate` is considered a wiring error.
- The presence of `storefront/Cargo.toml` without `[provides.storefront_ui].leptos_crate` is considered a wiring error.
- Declaring `[provides.admin_ui].leptos_crate` without an actual `admin/Cargo.toml` is considered an error.
- Declaring `[provides.storefront_ui].leptos_crate` without an actual `storefront/Cargo.toml` is considered an error.
- Versions of UI sub-crates must match the version of the main module.

The mere presence of an `admin/` or `storefront/` subfolder is not considered proof of integration. The canonical source of truth here is manifest wiring.

## Support and Capability Crates

Not every crate in the workspace is a platform module.

- Platform modules live in `modules.toml` and pass scoped validation through `cargo xtask module validate <slug>`.
- Foundation/shared/support/capability crates may have local docs and their own contracts, but are not required to have a slug in `modules.toml`.
- If a capability crate needs a formal runtime/module contract, it can be registered in `modules.toml` as a `capability_only` ghost module. Current live examples of this pattern: `alloy` and `flex`.

For such crates, the documentation minimum still applies:

- Root `README.md`;
- `docs/README.md` if needed;
- `docs/implementation-plan.md` if needed.

If a support/capability crate already publishes local docs, it is recommended to follow the same structural standard as platform modules: English root `README.md`, English `docs/README.md`, English `docs/implementation-plan.md`.

But they do not pass `module validate` until they become a platform module.

## How to Add a New Platform Module

`xtask` learns about a new platform module only from `modules.toml`. The presence of a crate in `crates/` alone does not make it a module.

Minimum order of addition:

1. Create a crate, typically `crates/rustok-<slug>/`, and ensure it is part of the Cargo workspace.
2. Add mandatory local documents: root `README.md`, `docs/README.md`, `docs/implementation-plan.md`.
3. Add `rustok-module.toml` with correct `module.slug`, `module.version`, `module.ui_classification`, dependency metadata and `[crate].entry_type` if the crate implements `RusToKModule`.
4. Add the slug to `[modules]` inside `modules.toml`; use `required = true` only for core modules, leave all other modules as optional.
5. Synchronize dependencies in three places: `modules.toml.depends_on`, `[dependencies]` in `rustok-module.toml`, `RusToKModule::dependencies()`.
6. For an optional runtime module, add a `mod-<slug>` feature and server wiring in `apps/server/Cargo.toml`.
   For a regular optional module this means `dep:<crate>`, and for a `capability_only` ghost module an empty feature-guard is acceptable if the crate is already always-linked as a shared server capability dependency.
7. For a required runtime module, add direct registration in `apps/server/src/modules/mod.rs`.
8. For module-owned UI, declare `[provides.admin_ui]` and/or `[provides.storefront_ui]` only together with an actual UI sub-crate and host wiring.
9. Update navigation: `docs/modules/_index.md`, `docs/modules/registry.md`, and for UI modules also `docs/modules/UI_PACKAGES_INDEX.md`.
10. Run local preflight: `cargo xtask validate-manifest`, `cargo xtask module validate <slug>`, `cargo xtask module test <slug>`.

The file template and minimum sections live in the [module documentation template](../templates/module_contract.md).

## Recommended Local Preflight

For a path module before publication or significant development, use:

```powershell
cargo xtask module validate blog
cargo xtask module test blog
```

If the entire platform composition contract changes, add:

```powershell
cargo xtask validate-manifest
```

## Related Documents

- [How to Write a Module in RusToK](./module-authoring.md)
- [Module and Application Registry](./registry.md)
- [Module Platform Crate Registry](./crates-registry.md)
- [Module Documentation Index](./_index.md)
- [Module Documentation Template](../templates/module_contract.md)
- [Main Verification README](../verification/README.md)

> Document status: current. When changing `xtask` rules, the acceptance contract for modules or the platform module composition, update this file together with `docs/index.md`.

## Runtime Snapshot and Manifest Hash

`modules.toml` remains a declarative bootstrap/dev manifest, but production runtime reads the active composition from
`platform_state`. During install/uninstall/upgrade, the control plane stores the full manifest JSON snapshot and SHA-256 hash
of this snapshot. The hash is computed from the canonical JSON of the entire manifest, not just the module list, so changes
to `settings`, build profile, source pins and dependency metadata change the immutable artifact identity.
