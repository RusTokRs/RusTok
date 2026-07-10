---
id: doc://docs/architecture/loco-exit-plan.md
kind: implementation_plan
language: markdown
status: draft
---
# Loco RS Exit Plan

This document captures the single repo-level plan for migrating `apps/server` from Loco RS to a pure Axum runtime and own CLI/maintenance entrypoints.

The plan replaces the previous direction of "integrate Loco deeper". The old `apps/server/docs/loco-core-integration-plan.md` roadmap has been removed. `apps/server/docs/LOCO_FEATURE_SUPPORT.md` remains as historical inventory only and is not the target roadmap.

## Execution Checkpoint

- Current phase: `phase_1_runtime_context_and_request_extractors`
- Last checkpoint: introduced `rustok_api::HostRuntimeContext`, `apps/server` passes it into Leptos `#[server]` functions; `rustok-index-admin`, `rustok-outbox-admin`, `rustok-channel-admin`, `rustok-ai-admin`, `rustok-search-admin`, `rustok-product` admin/storefront, `rustok-seo-admin`, `rustok-mcp-admin`, `rustok-inventory-admin` and `rustok-cart-storefront` native transports no longer import `loco_rs::app::AppContext`. `rustok-ai-admin` and `rustok-search-admin` resolve their required DB/event handles through `HostRuntimeContext`, with no `loco-rs` or `rustok-outbox/loco-adapter` package dependency; `rustok-commerce-storefront` no longer carries a package-level `loco-rs` dependency after its native code had already moved to owner checkout runtimes; backend foundation crates `rustok-runtime`, `rustok-web`, `rustok-fba`, `rustok-cli-core`, `rustok-cli`, `rustok-cli-platform` and `rustok-cli-registry` were added so executable runtime/web/FBA/CLI helpers do not continue to expand `rustok-api` or `apps/server`; `rustok-cli-registry` is now backed by generated source from root `cli-registry.toml` and module `[provides.cli]` metadata; `rustok-cli-platform` provides `core version` through `CommandProvider::execute`; `apps/server/src/controllers/health.rs`, `apps/server/src/controllers/users.rs`, `apps/server/src/controllers/channel.rs` and `apps/server/src/controllers/auth.rs` now use `rustok_web::json_response` instead of Loco response formatting while keeping Loco `Routes` for the later Axum router slice; `scripts/verify/verify-loco-inventory.mjs` classifies remaining Loco entrypoints; ADR `2026-07-02-axum-runtime-and-ops-cli-boundary` accepted.
- Runtime composition note: `apps/server` now supplies `Arc<ModuleRuntimeExtensions>` as a typed `HostRuntimeContext` handle to native server functions. `rustok-auth-admin` consumes that handle for owner mutation runtimes and its database handle for owner reads, without accessing the server Loco shared store.
- Next step: verify the first real module-local provider, `rustok-media-cli` / `media cleanup`, then remove the matching Loco task. Its adapter explicitly bootstraps `StorageService` from the CLI `storage` settings snapshot and uses the database supplied by `RuntimeComposition`; it does not recreate server shared-store initialization. Continue server controller response/error boundary slices through `rustok-web` when touching controllers.
- Open blockers: none for Phase 1 planning; before Phase 4, targeted integration smoke for pure Axum startup will be needed.
- Hand-off notes for next agent: do not add compatibility wrappers and dual execution paths; each cutover must migrate all internal callers to the target contract and remove the replaced Loco path in the same change set.
- Last updated at (UTC): 2026-07-10T00:00:00Z

## Goal

Target state:

1. `apps/server` starts as a pure Axum application without `loco_rs::cli`, `Hooks`, `AppContext`, `Routes`, Loco tasks and Loco initializers.
2. Runtime context belongs to RusToK: typed DB handle, settings, shared runtime registry, event/outbox/cache/storage/email handles, shutdown handles and observability hooks are available through own host contracts.
3. Operator CLI belongs to RusToK, but lives separately from the server runtime: a separate CLI crate/binary calls typed Rust APIs for migrate, seed, install and maintenance flows; `apps/server` does not depend on this CLI layer.
4. Modules and UI packages do not import `loco_rs`; they receive host data through `rustok-api`, module ports, GraphQL, REST or native `#[server]` context.
5. Workspace dependency `loco-rs` is removed from `Cargo.toml` and `Cargo.lock`.

## Not In Scope

- Replacing Axum, SeaORM, async-graphql or Leptos.
- Removing GraphQL when adding native `#[server]` paths.
- Moving domain logic to `apps/server`.
- Introducing a second temporary runtime alongside Loco indefinitely.
- Keeping Loco-compatible aliases for the sake of internal callers.

## What Has Already Been Migrated

| Area | Current State | Target Owner |
|---|---|---|
| Auth/JWT/password/session domain | Implemented in `rustok-auth` and server services; Loco JWT is not the source of truth | `rustok-auth` + server auth adapter |
| RBAC | Runtime policy moved to `rustok-rbac`/shared contracts | `rustok-rbac`, `rustok-api`, `apps/server` enforcement |
| Cache | `rustok-cache` and tenant cache infra are used instead of Loco cache | `rustok-cache`, tenant middleware |
| Storage | `rustok-storage` + `rustok-media`; server bootstrap initializes `StorageService` | `rustok-storage`, `rustok-media`, server wiring |
| Email | `rustok-email` and server email service cover SMTP/templates; Loco provider still remains an option | `rustok-email` + server adapter |
| Queue/event delivery | Transactional outbox and relay are the source of truth; Loco Queue is not used | `rustok-outbox`, `rustok-events`, server workers |
| WebSocket channels | Custom Axum WS path is used, not Loco channels | `apps/server` + channel/auth modules |
| Module-owned API composition | GraphQL/REST are increasingly assembled through manifests and owner-owned roots | module crates + generated server composition |
| Leptos server-function context | Migration to `rustok_api::HostRuntimeContext` started; `index/outbox/tenant/region/comments/workflow/media/customer/channel/ai/product/seo/mcp/inventory` admin and `region/product/cart` storefront already migrated; media and AI also use host-provided typed shared handles instead of Loco `shared_store` | `rustok-api` + server context provider |
| Installer CLI | `rustok-server install ...` already exists as its own CLI slice; target state is migration to a separate platform CLI binary | `rustok-installer` + CLI adapter |

## What Still Holds Loco

| Remnant | Examples of Current Points | Replacement |
|---|---|---|
| Application bootstrap | `apps/server/src/main.rs`, `apps/server/src/app.rs`, `impl Hooks for App` | `serve()` on Axum `Router`, own lifecycle bootstrap |
| Loco `AppContext` | controllers, GraphQL data, middleware, tasks, services, tests, module UI adapters | `ServerRuntimeContext` / `HostRuntimeContext` / typed request extractors |
| Loco route wrappers | `loco_rs::controller::Routes`, `format`, `ErrorDetail`, `loco_rs::Result` | Axum `Router`, typed response/error mappers |
| Loco config | `loco_rs::config::Config`, `config/*.yaml` conventions | `RustokSettings` loader + explicit env/file contract |
| Loco tasks | `cargo loco task --name ...`, `loco_rs::task::{Task, Vars}` | separate `rustok-cli task <name>` / typed subcommands |
| Loco initializers | `loco_rs::app::Initializer` | explicit bootstrap phases with ordered init functions |
| Loco test helpers | `loco_rs::tests_cfg::app::get_app_context` | `rustok-test-utils` server context fixtures |
| Loco mailer option | `EmailProvider::Loco`, `ctx.mailer` | remove provider or replace with native SMTP/provider adapter |
| Outbox Loco adapter | `rustok-outbox/loco-adapter`, `rustok_outbox::loco` | host-neutral event bus factory |
| Docs/verification | Loco integration plans and Loco-specific guards | Axum/platform CLI cutover guards and archived Loco docs |

## Remaining Scope Estimate

The July 2026 inventory is a size signal, not a task counter: many occurrences are guardrails, docs, lockfile entries, tests or transitional route adapters. The remaining work should be planned as architectural slices, not individual text matches.

Current classified inventory baseline after the `rustok-ai-admin` native transport cutover:

| Category | Count | Practical Meaning |
|---|---:|---|
| `host_runtime` | 38 | Server bootstrap, app lifecycle, runtime context boundary and mailer/runtime bridges. |
| `module_ui_adapter` | 90 | Largest remaining non-core surface: module-owned Leptos/native adapters still reading `AppContext`. |
| `module_controller` | 35 | Mostly controller route/state adapters and remaining Loco controller API usage after handler runtime narrowing. |
| `server_task` / `server_seed` / `server_schedule` | 5 | Maintenance flows that belong in `rustok-cli`, not the HTTP server binary; task and seed framework imports are now isolated behind local server bridges and active server comments no longer advertise `cargo loco` execution. |
| `server_test` | 16 | Loco test fixtures to replace with `rustok-test-utils` server/runtime fixtures. |
| `dependency_manifest` / `lockfile` | 47 | Cleanup after code paths stop requiring `loco-rs` and `loco-adapter`. |
| `verification_guard` / `docs` / `scaffold_template` | 422 | Guardrails, historical docs and generated templates to update/archive last; crate docs and verifier guardrails grow as boundaries are made explicit. |

Approximate remaining effort:

| Workstream | Share of Remaining Work | Notes |
|---|---:|---|
| Server core bootstrap/routing/tasks/config | 40-50% | `apps/server/src/app.rs`, `main.rs`, hooks, initializers, tasks, seeds, Loco `Routes`, error/response contracts. |
| Module UI/native adapters | 30-35% | Broad but repetitive; should be handled package-by-package with guardrails. |
| Module controller route adapters | 10-15% | Many handlers already use narrow runtimes; remaining work is mostly Axum route/error cutover. |
| Dependency cleanup, tests, docs, scaffolds | 10-15% | Runs after production code paths are no longer Loco-bound. |

Expected execution size: roughly 24-39 focused cutover slices if each slice migrates one coherent runtime or package boundary and updates guardrails/docs in the same change.

## Loco Integration Documents Policy

There is no longer a target "Loco integration" plan. Historical Loco integration documents are inventory/context only and must not be used as implementation guidance.

Policy:

- replace active Loco integration plans with this exit plan and the Axum/platform CLI ADR;
- mark remaining Loco-specific docs as `deprecated` or `archived` when touched;
- keep only short historical notes that explain what Loco surface was replaced and by which RusToK-owned contract;
- do not add new Loco compatibility docs, aliases or dual internal execution paths;
- remove Loco docs/scaffold references in the final dependency cleanup phase after code no longer imports Loco.

## Extraction Targets

Some replacement code should not accumulate in `apps/server`. The target architecture is a small server composition root plus owned foundation crates and module adapters.

| Target | Proposed Home | Why It Is Separate |
|---|---|---|
| CLI capability contract | `crates/rustok-cli-core` | Stable command/provider descriptions without depending on CLI binary, server or domain internals. |
| CLI runner binary | `crates/rustok-cli` with binary `rustok-cli` | Maintenance process, argument parsing, help/search UX and runtime construction outside the HTTP server. |
| CLI registry | `crates/rustok-cli-registry` | Distribution-specific aggregation of selected module command providers; generated from module `[provides.cli]` metadata and currently empty until provider metadata lands. |
| Module ops adapters | `crates/rustok-<module>/cli` or external module `cli/` package | Keeps commands beside their owner while keeping executable maintenance code out of domain core and server runtime. |
| Server runtime foundation | `crates/rustok-runtime` | Shared runtime helpers and typed host handles move out of `rustok-api` as they become executable wiring instead of stable API contracts. |
| Axum adapter utilities | `crates/rustok-web` | Replacement for Loco `Routes`, response formatting and error mapping that modules can use without depending on `apps/server`. |
| FBA metadata contracts | `crates/rustok-fba` | Shared provider/consumer registry metadata, topology and transport-profile descriptors without owning transport implementations. |
| Test runtime fixtures | `crates/rustok-test-utils` | Replaces `loco_rs::tests_cfg` and gives modules/server a shared neutral runtime fixture. |
| Outbox Loco adapter | temporary `rustok-outbox` feature only | Kept only as a bridge until the final imports are gone; it must not grow new responsibilities. |

Extraction rule: extract only stable cross-boundary contracts or executable adapters. Do not move domain logic to server/foundation crates, and do not make module domain crates depend on CLI concerns.

Backfill rule: do not rewrite already-green slices such as AI, product, SEO, MCP, inventory or cart just to consume a new helper. Backfill is allowed only when the file is already changing for a Loco/FBA boundary task, the helper is already used in at least two places, and a guardrail prevents reintroducing local duplication.

## Target Runtime Contract

### Server Bootstrap

Target entrypoint:

1. load settings;
2. connect DB;
3. optionally validate required schema/runtime preconditions;
4. build `ServerRuntimeContext`;
5. register module runtime extensions;
6. build event/outbox/cache/storage/email/telemetry runtimes;
7. compose Axum router;
8. start HTTP server and shutdown handling.

`apps/server/src/app.rs` must stop being a Loco hooks layer and become an ordinary Axum composition module or be split into `bootstrap`, `router`, `lifecycle`.

### Runtime Context

Minimum own context:

- `db: DatabaseConnection`
- `settings: Arc<RustokSettings>`
- `registry: ModuleRegistry`
- `runtime_extensions: Arc<ModuleRuntimeExtensions>`
- `shared`: typed runtime store only if ownership is explicit
- event bus / event transport
- cache/storage/email services
- shutdown handles
- build/release/installer runtimes

Rule: module-owned UI and transport adapters do not receive this full context directly. Narrow contracts are used for them: `HostRuntimeContext`, `RequestContext`, `PortContext`, GraphQL data or module-owned facades.

### Operator CLI

The CLI remains as an external operator/dev interface for migrations, seed, install and maintenance flows. It is not an internal integration layer: business logic, module contracts and runtime wiring must live in typed Rust APIs, and commands only call these APIs and set the execution mode.

Target owner: a separate crate/binary, for example future `crates/rustok-cli` with binary `rustok-cli`. Shared command/provider contracts live in `crates/rustok-cli-core`. `apps/server` must not depend on the CLI binary crate, and the production HTTP binary must not carry maintenance command code in its build.

The domain core of a module does not depend on `clap`/stdout/exit-code contracts. If a module needs a maintenance flow, the module may own a separate `cli/` adapter package alongside the domain code; this adapter calls the module's public typed APIs and does not participate in the server runtime build. For external or rewritten modules, such an adapter may be shipped with the module or live in the integration layer, but the registry connects it the same way.

#### Scalable Ops Command Model

`rustok-cli` must not turn into a catalog of hardcoded commands for all modules. Target structure:

- `rustok-cli-core`: small stable contract for describing CLI capabilities, arguments, permissions, dry-run mode, tenant scope, asynchronous typed execution requests and machine-readable results.
- `rustok-cli`: runner, parser, help/list/search UX, settings loading and CLI runtime context construction. The initial runner crate exists and currently owns built-in help/list behavior, namespace-scoped discovery, namespace command dispatch, normalized command arguments, explicit duplicate command detection and `list --json` machine-readable inventory output only.
- `rustok-cli-platform`: platform-level command provider crate for commands not owned by a domain module.
- `rustok-cli-registry`: explicit registry of connected command providers for a specific build/distribution. The crate exists with generated selected-distribution source from root `cli-registry.toml` and module `[provides.cli]` metadata.
- `crates/rustok-<module>/cli`: module-local ops adapter package that maps an ops command provider to the module's typed API.
- `integrations/<external-module>/cli`: adapter package for external modules, if the provider does not ship its own module-local CLI adapter.

The domain crate does not depend on `rustok-cli-core`; the dependency is from the `cli/` adapter package to the domain crate. Physically, the adapter lives alongside the module so that commands, scripts and maintenance crud do not accumulate in the central CLI crate, but architecturally it is an inbound adapter, not a domain core.

Command names are namespace-based, without a flat global dump:

| Format | Example | Purpose |
|---|---|---|
| `rustok-cli <namespace> <command>` | `rustok-cli index rebuild` | module-owned maintenance |
| `rustok-cli core <command>` | `rustok-cli core migrate` | platform/core operations |
| `rustok-cli list --namespace index` | - | discoverability without a huge root help |

Provider registration must be explicit: through module manifest, feature/distribution manifest or generated registry, not through runtime magic. This preserves build reproducibility, allows any number of modules without manual central mapping, and makes it possible to ship a production server without the CLI layer, and a CLI binary with only the needed command providers.

#### Distribution-aware Builds

`rustok-cli` in the target state may become not just a maintenance runner, but also a build/pack/install toolchain for the platform from a selected set of modules. This is needed for builds with custom modules from other participants without rigid changes to the central server crate.

Principle:

- distribution manifest describes a set of core, internal and external modules, their features, migrations, seeds, runtime entrypoints and ops providers;
- generated registry is created from the manifest/lockfile and fixes which runtime modules and CLI providers participate in a specific build;
- server build receives only the runtime parts of the selected distribution;
- ops build receives only the ops providers of the selected distribution;
- external modules are connected through a published module manifest and optional `cli/` adapter package, not through manual code in the central repository.

Possible future toolchain commands:

| Command | Purpose |
|---|---|
| `rustok-cli distro check` | check manifests, features, migrations, command namespaces |
| `rustok-cli distro generate` | generate runtime/CLI registries for the selected build |
| `rustok-cli distro build server` | build server binary without CLI layer |
| `rustok-cli distro build cli` | build CLI binary with selected providers |
| `rustok-cli distro pack` | prepare installable artifact |

Target commands:

| Command | Purpose | Replaces |
|---|---|---|
| `rustok-server` / server binary | HTTP runtime only | `cargo loco start` / `loco_rs::cli` |
| `rustok-cli migrate up/down/status` | DB migrations | Loco migration wrapper |
| `rustok-cli seed <profile>` | seed profiles | Loco `seed` hook |
| `rustok-cli task cleanup ...` | cleanup maintenance | `cargo loco task --name cleanup` |
| `rustok-cli task rebuild ...` | rebuild/index maintenance | `cargo loco task --name rebuild` |
| `rustok-cli task db-baseline ...` | DB baseline report | `cargo loco task --name db_baseline` |
| `rustok-cli media cleanup [--limit <count>]` | storage/media cleanup | `cargo loco task --name media_cleanup` |
| `rustok-cli oauth create-app ...` | OAuth app bootstrap | `cargo loco task --name create_oauth_app` |
| `rustok-cli install ...` | install/preflight/apply | existing Rustok install CLI path |

## Migration Phases

### Phase 0. Inventory and Prohibition of New Loco Surface

- [x] Introduce `rustok_api::HostRuntimeContext`.
- [x] Migrate first module UI adapters (`rustok-index-admin`, `rustok-outbox-admin`, `rustok-tenant-admin`, `rustok-region-admin`, `rustok-comments-admin`, `rustok-workflow-admin`) from Loco context.
- [x] Add guardrail in `scripts/verify/verify-api-surface-contract.mjs` for these adapters.
- [x] Add a general inventory script: all `loco_rs`, `loco-rs`, `cargo loco`, `rustok_outbox::loco` with categorization of current host/runtime/task/test/module/docs/dependency points.
- [x] Prohibit new `loco_rs` imports outside the allowlist.
- [x] Capture ADR for target Axum runtime and separate platform CLI layer.

Exit gate: inventory script passes, allowlist is fixed, new Loco imports without a category fail in CI.

### Phase 1. Own Runtime Context and Request Extractors

- [x] Introduce `ServerRuntimeContext` in `apps/server` or shared server-support crate.
- [x] Migrate middleware to own context instead of `loco_rs::app::AppContext`.
- [x] Migrate GraphQL implementation and controller handlers to own context and narrow runtime handles: `apps/server/src/graphql/**` and `controllers/graphql.rs` no longer import Loco `AppContext`; Loco `Routes` remains a routing adapter until Phase 2.
- [x] Migrate host-owned GraphQL query/mutation roots and settings/system/user fields/build subscription to neutral GraphQL data: build/marketplace paths use `ServerRuntimeContext`, DB-only paths use schema-owned `DatabaseConnection`, settings mutation uses schema-owned `TransactionalEventBus`; HTTP/WS controller no longer adds Loco host data to GraphQL requests.
- [x] Migrate first shared-store services (`BuildEventHub`, `FieldDefinitionCache`, `MarketplaceCatalog`) to `ServerRuntimeContext`; GraphQL/channel/build boundaries still construct context from current Loco host data.
- [x] Migrate `EventBus` and server-side `TransactionalEventBus` factory to `ServerRuntimeContext`; crate-local `rustok_outbox::loco` adapter and dependency feature remain a separate Phase 5 removal item.
- [x] Migrate runtime guardrail snapshot service, health readiness and metrics controllers to `ServerRuntimeContext` + narrow email runtime state.
- [x] Migrate RBAC consistency stats service and metrics caller to `ServerRuntimeContext`; legacy cleanup task still remains a task boundary adapter.
- [x] Migrate release deployment backend to `ServerRuntimeContext`; build worker lifecycle still remains a boundary adapter.
- [x] Migrate build executor service to `ServerRuntimeContext`; build worker and legacy rebuild task still remain boundary adapters.
- [x] Migrate event transport factory to `ServerRuntimeContext`; app runtime bootstrap still remains a boundary adapter.
- [x] Migrate spawn path module event dispatcher to `ServerRuntimeContext`; host-provider wiring still remains a boundary adapter due to auth lifecycle provider.
- [x] Migrate email service factory/password reset URL to `ServerRuntimeContext`; Loco mailer remains an explicit boundary handle.
- [x] Migrate app runtime bootstrap helpers (`module_runtime_extensions_from_ctx`, storage, marketplace catalog, workflow cron shared setup) to `ServerRuntimeContext`; Loco-specific bootstrap remains a boundary adapter.
- [x] Migrate rate-limit bootstrap and shared limiter registration to `ServerRuntimeContext`; Loco `AppContext` in `bootstrap_app_runtime` remains only for current Loco-boundary adapters.
- [x] Isolate `app_runtime` and `app_router` Loco host context imports behind local host bridge aliases; the pure Axum bootstrap cutover still owns the actual removal of these aliases.
- [x] Isolate auth config host context behind `crate::auth::AuthHostContext`; `auth.rs` no longer imports Loco `AppContext` directly or describes auth errors as direct Loco errors.
- [x] Migrate `check_production_secrets` failures to `crate::error::Error::Message`; `app.rs` no longer constructs `loco_rs::Error::Message` directly.
- [x] Migrate GraphQL schema assembly, shared/cache helpers and media storage fallback to `ServerRuntimeContext`; Loco host data remains only above, in the current app bootstrap/controller boundary.
- [x] Remove Loco `AppContext` from `rustok-content-orchestration`: host builds `SharedContentOrchestrationService` from explicit DB/event bus handles, GraphQL receives it through schema data.
- [x] Migrate Alloy runtime bootstrap, GraphQL resolvers and HTTP handlers to explicit runtime handles: host builds `SharedAlloyRuntime` via `alloy::build_alloy_runtime(DatabaseConnection)`, GraphQL reads this handle from schema-owned data, REST handlers receive `AlloyHttpRuntime`, and Loco remains only in the current route-state adapter until Phase 2/5.
- [x] Remove Loco `AppContext` from `rustok-ai`: GraphQL mutation, `AiManagementService`, direct execution handlers and in-process MCP adapter use `AiHostRuntime` with explicit DB/event bus/storage/Alloy/module-registry handles.
- [x] Remove Loco `AppContext` from `rustok-commerce` storefront checkout runtime: owner storefront SSR adapters collect `StorefrontCheckoutRuntime` from DB/event bus at their boundary, and checkout orchestration API accepts only this host-neutral contract.
- [x] Narrow the first `rustok-commerce` product HTTP slice: `controllers/products.rs` and `controllers/admin/products.rs` accept `CommerceHttpRuntime` and do not use `rustok_outbox::loco`; remaining commerce admin/storefront adapters remain as next Loco-boundary slices.
- [x] Narrow `rustok-commerce` storefront product/catalog HTTP slice: `controllers/store/products.rs` accepts `CommerceHttpRuntime` for product list/show, regions and shipping-options reads; the file no longer accepts Loco `AppContext` and does not use `rustok_outbox::loco`.
- [x] Narrow `rustok-commerce` storefront order HTTP slice: `controllers/store/orders.rs` accepts `CommerceHttpRuntime` for customer/order/return/refund/change reads and return creation; the file no longer accepts Loco `AppContext` and does not use `rustok_outbox::loco`.
- [x] Narrow `rustok-commerce` storefront cart HTTP slice: `controllers/store/carts.rs` accepts `CommerceHttpRuntime` for create/get/context/line-item routes and no longer accepts Loco `AppContext` or `rustok_outbox::loco`.
- [x] Narrow `rustok-commerce` storefront checkout HTTP slice: `controllers/store/checkout.rs` accepts `CommerceHttpRuntime` for payment-collection and complete checkout routes; `controllers/store/mod.rs` no longer holds `rustok_outbox::loco` helper wrappers.
- [x] Narrow `rustok-commerce` admin fulfillment HTTP slice: `controllers/admin/fulfillments.rs` accepts `CommerceHttpRuntime` for list/create/show/ship/deliver/reopen/reship/cancel and no longer accepts Loco `AppContext`.
- [x] Narrow `rustok-commerce` admin shipping HTTP slice: `controllers/admin/shipping.rs` accepts `CommerceHttpRuntime` for shipping profiles/options list/create/show/update/deactivate/reactivate and no longer accepts Loco `AppContext`.
- [x] Narrow `rustok-commerce` admin payment HTTP slice: `controllers/admin/payments.rs` accepts `CommerceHttpRuntime` for payment collections/refunds list/show/lifecycle routes and no longer accepts Loco `AppContext`.
- [x] Narrow `rustok-commerce` admin order HTTP slice: `controllers/admin/orders.rs` accepts `CommerceHttpRuntime` for order list/detail/lifecycle routes and no longer uses `rustok_outbox::loco`.
- [x] Narrow `rustok-commerce` admin order-change HTTP slice: `controllers/admin/changes.rs` accepts `CommerceHttpRuntime` for create/list/show/apply/cancel and no longer uses `rustok_outbox::loco`.
- [x] Narrow `rustok-commerce` admin return HTTP slice: `controllers/admin/returns.rs` accepts `CommerceHttpRuntime` for create/list/show/complete/cancel/decision routes and no longer uses `rustok_outbox::loco`.
- [x] Migrate `rustok-blog` HTTP route entrypoint to manifest-declared `controllers::axum_router`: it receives `HostRuntimeContext`, builds `BlogHttpRuntime` from DB plus typed `TransactionalEventBus`, and is merged once by generated host Axum composition. Post/comment handlers use `rustok_web::HttpError`; the module no longer depends on Loco or exposes a Loco route-state adapter.
- [x] Migrate `rustok-pages` HTTP route entrypoint to manifest-declared `controllers::axum_router`: page/block handlers use `PagesHttpRuntime` from `HostRuntimeContext` plus typed `TransactionalEventBus`, and `rustok_web::HttpError` response errors. The module no longer depends on Loco or exposes a route-state adapter.
- [x] Migrate `rustok-forum` HTTP route entrypoint to manifest-declared `controllers::axum_router`: handlers receive `ForumHttpRuntime` from `HostRuntimeContext` plus typed `TransactionalEventBus` and use `rustok_web::HttpError`; legacy Loco routes and dependency are removed.
- [x] Migrate `rustok-commerce` HTTP route entrypoint to manifest-declared `controllers::axum_router`: owner-owned store/admin Axum routers receive `CommerceHttpRuntime` from `HostRuntimeContext` plus typed `TransactionalEventBus`; production handlers and transport test fixtures use `rustok_web` errors and neutral runtime fixtures without Loco.
- [x] Migrate `rustok-media` HTTP route entrypoint to manifest-declared `controllers::axum_router`: handlers receive `MediaHttpRuntime` from `HostRuntimeContext` plus typed `StorageService` and use `rustok_web::HttpError`; legacy Loco routes and dependency are removed.
- [x] Migrate `rustok-workflow` HTTP and webhook entrypoints to manifest-declared `controllers::axum_router` and `controllers::axum_webhook_router`: workflow/step/execution/webhook handlers receive `WorkflowHttpRuntime` from `HostRuntimeContext`; legacy Loco routes, server shim and dependency are removed.
- [x] Migrate `rustok-seo` HTTP route entrypoint to manifest-declared `controllers::axum_router`: handlers receive DB, typed event bus and `ModuleRuntimeExtensions` through `HostRuntimeContext`; the module keeps its own SEO REST error envelope and removes Loco routes/dependency.
- [x] Migrate runtime worker lifecycle orchestration to `ServerRuntimeContext` for settings/shared-store/event-runtime lookup; worker loops still remain boundary adapters where they need `AppContext`.
- [x] Migrate DB-only paths auth lifecycle provider (`list_sessions`, `update_profile`, `change_password`, `logout`, session revoke, accept invite user create) to `ServerRuntimeContext`.
- [x] Migrate config-aware auth lifecycle provider paths (`login`, `register`, `refresh`, `reset_password`) to `ServerRuntimeContext` + explicit `AuthConfig`; `AuthConfig` assembly still remains a boundary adapter to the current Loco config.
- [x] Remove full Loco `AppContext` from `ServerAuthLifecycleProvider` and runtime extension assembly; bootstrap passes explicit `ServerRuntimeContext`, `AuthConfig` and narrow mailer handle.
- [x] Migrate REST auth controller to `ServerAuthRuntime`/`ServerEmailRuntime`, runtime/config entrypoints and remove superseded Loco `AppContext` entrypoints from `AuthLifecycleService`.
- [x] Migrate `tenant`, `channel` and `locale` middleware to `ServerRuntimeContext`; router/health/metrics remain boundary adapters.
- [x] Migrate `auth_context`, `CurrentUser`/`OptionalCurrentUser` and RBAC permission extractor macro to narrow `ServerAuthRuntime`; Loco `AppContext` remains only as a boundary source for assembling this runtime in the current host.
- [x] Migrate module guard and server channel contract to `ServerRuntimeContext`; Loco context is no longer a public request/channel contract inside the server.
- [x] Migrate GraphQL and users controller handlers to Axum substate (`ServerRuntimeContext`/`ServerAuthRuntime`); Loco `Routes` and error contracts remain for Phase 2 routing cutover.
- [x] Migrate metrics handler and helper pipeline to `ServerRuntimeContext` + narrow `ServerEmailRuntime`; non-clone worker handles are read through scoped runtime API without leaking Loco `SharedStore`.
- [x] Migrate health readiness/runtime handlers and dependency checks to `ServerRuntimeContext` + `ServerEmailRuntime`; Loco route assembly remains a separate Phase 2 item.
- [x] Migrate channel and standalone Flex REST handlers to `ServerRuntimeContext`; channel controller also uses `rustok_web::json_response` for JSON response formatting, while Flex controller tests use neutral runtime fixture instead of test-only Loco `AppContext`.
- [x] Migrate OAuth metadata handler to `ServerAuthRuntime`; discovery metadata no longer reads auth config from Loco host state.
- [x] Migrate OAuth REST token, authorize/consent, browser-session and revoke handlers to `ServerAuthRuntime`/`ServerRuntimeContext`; Loco `Routes` remains only a routing adapter until Phase 2.
- [x] Migrate auth controller JSON response formatting from `loco_rs::controller::format` to `rustok_web::json_response`; Loco `Routes` remains for the router cutover slice.
- [x] Migrate marketplace registry/governance REST handlers to `ServerRuntimeContext`; artifact storage and remote executor settings are read through neutral runtime state.
- [x] Migrate swagger, installer status/receipts, admin DLQ, MCP management/remote tools and build WebSocket handlers to `ServerRuntimeContext`; Loco `Routes` remains only a routing adapter until Phase 2.
- [ ] Migrate Leptos server functions to `HostRuntimeContext`/typed narrow contexts.
- [x] Migrate `rustok-tenant-admin` native bootstrap server function to `HostRuntimeContext` and remove its `loco-rs` dependency.
- [x] Migrate `rustok-region-admin` native CRUD server functions to `HostRuntimeContext` and remove its `loco-rs` dependency.
- [x] Migrate `rustok-comments-admin` native moderation server functions to `HostRuntimeContext` and remove its `loco-rs` dependency.
- [x] Migrate `rustok-workflow-admin` native workflow server functions to `HostRuntimeContext` and remove its `loco-rs` dependency.
- [x] Migrate `rustok-media-admin` native media server functions to `HostRuntimeContext`, provide `StorageService` through the neutral typed host-handle snapshot, and remove its `loco-rs` dependency.
- [x] Migrate `rustok-customer-admin` native customer CRUD server functions to `HostRuntimeContext` and remove its `loco-rs` dependency.
- [x] Migrate `rustok-region-storefront` native region discovery server function to `HostRuntimeContext` and remove its `loco-rs` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-channel-admin` native channel management server functions to `HostRuntimeContext` and remove its `loco-rs` dependency while preserving the REST secondary path.
- [x] Migrate `rustok-ai-admin` native control-plane server functions to `HostRuntimeContext`, provide `TransactionalEventBus`, AI registry, storage and Alloy runtime through neutral typed host handles, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-search-admin` native search control-plane server functions to `HostRuntimeContext`, provide `TransactionalEventBus` only to event-publishing flows through a neutral typed host handle, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-auth-admin` native user/OAuth server functions to `HostRuntimeContext`, provide `Arc<ModuleRuntimeExtensions>` through a neutral typed host handle, and remove its `loco-rs` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-commerce-admin` native cart-promotion and order-change server functions to `HostRuntimeContext`, provide `TransactionalEventBus` through the existing neutral typed host handle, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-pricing-admin` native price-list, variant-price and discount server functions to `HostRuntimeContext`, provide `TransactionalEventBus` through the existing neutral typed host handle, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-pages-storefront` native page-read server function to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-blog-storefront` native post-read server function to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-order-storefront` native checkout-completion server function to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-fulfillment-storefront` native shipping-selection server function to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-payment-storefront` native payment collection/refund server functions to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-pricing-storefront` native pricing atlas server function to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-product-admin` native category-bound catalog server functions to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-product-storefront` native catalog read and public catalog-search option server functions to `HostRuntimeContext`, provide `TransactionalEventBus` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-seo-admin` native SEO control-plane server functions to `HostRuntimeContext`, provide `TransactionalEventBus` and `ModuleRuntimeExtensions` through the neutral typed host-handle snapshot, and remove its `loco-rs` / `loco-adapter` dependency.
- [x] Migrate `rustok-mcp-admin` native MCP control-plane server functions to `HostRuntimeContext`, provide DB and `ModuleRuntimeExtensions` through neutral typed host handles, and remove its `loco-rs` dependency while preserving GraphQL selected path.
- [x] Migrate `rustok-inventory-admin` native read/write server functions to `HostRuntimeContext`, provide `TransactionalEventBus` through neutral typed host handles, and remove its `loco-rs` / `loco-adapter` dependency.
- [x] Remove the stale `loco-rs` package dependency from `rustok-commerce-storefront` after the aggregate checkout code had already moved to Loco-free owner runtimes.
- [ ] Migrate module/capability crates where `loco_rs::app::AppContext` is currently used as a service locator.

Exit gate: module-owned crates and UI packages do not import `loco_rs::app::AppContext`; Loco context remains only in server bootstrap/tests allowlist.

### Phase 2. Axum Routing and Errors Without Loco Controller API

- [ ] Replace `loco_rs::controller::Routes` with Axum `Router` in server controllers.
- [ ] Introduce unified `AppError` / response mapper without `loco_rs::Error`.
- [ ] Migrate `crate::error::{Error, Result}` off `pub use loco_rs`.
- [x] Migrate health controller JSON response formatting from `loco_rs::controller::format` to `rustok_web::json_response` while keeping Loco `Routes` for the router cutover slice.
- [x] Migrate users controller JSON response formatting from `loco_rs::controller::format` to `rustok_web::json_response` while keeping Loco `Routes` for the router cutover slice.
- [x] Migrate channel controller JSON response formatting from `loco_rs::controller::format` to `rustok_web::json_response` while keeping Loco `Routes` for the router cutover slice.
- [x] Isolate users/channel forbidden error details behind `crate::error::http_error(rustok_web::HttpError)` so controllers no longer build Loco `ErrorDetail` directly.
- [x] Isolate installer HTTP error details behind `crate::error::http_error(rustok_web::HttpError)` while keeping Loco `Routes` for the router cutover slice.
- [x] Isolate marketplace registry authorization/governance error details behind `crate::error::http_error(rustok_web::HttpError)` while keeping Loco `Routes` for the router cutover slice.
- [x] Isolate server-owned controller route declarations behind `crate::routes::Routes`; `apps/server/src/routes.rs` is the only server controller bridge to Loco `Routes` until the Axum router composition cutover.
- [x] Isolate server `AppRoutes` composition and generated optional-module route signature behind `crate::routes::AppRoutes`; full Axum `Router` composition remains the next router cutover.
- [x] Isolate default-route creation and route mounting behind `crate::routes::{default_app_routes, mount_route}` so `app.rs` and generated optional-route code no longer call Loco `AppRoutes` methods directly.
- [x] Migrate auth controller JSON response formatting from `loco_rs::controller::format` to `rustok_web::json_response` while keeping Loco `Routes` for the router cutover slice.
- [x] Establish the first atomic module Axum router cutover with `rustok-blog`: `[provides.http].axum_router` is mutually exclusive with legacy `routes`; generated host composition merges only the declared module router after `HostRuntimeContext` is assembled.
- [x] Apply the atomic manifest-declared Axum router contract to `rustok-pages`; its legacy `Routes` and Loco error dependency are removed rather than mounted in parallel.
- [ ] Migrate health/metrics/graphql/auth/controllers to Axum response contracts.
- [ ] Update OpenAPI/export reference gates.

Exit gate: production HTTP/GraphQL routes are assembled without `loco_rs::controller::*`.

### Phase 3. Separate Ops CLI for Tasks, Seeds, Migrations

- [ ] Leave server binary responsible only for HTTP runtime startup/shutdown.
- [x] Introduce `rustok-cli-core` for stable CLI capability/provider contracts.
- [x] Isolate server task trait, task metadata, variables and task `AppContext` imports behind `apps/server/src/tasks/mod.rs`; concrete Loco task registration remains until the separate `rustok-cli` cutover.
- [x] Isolate server seed `AppContext` and seed error mapping behind `apps/server/src/seeds/mod.rs`; concrete Loco seed execution remains until the separate `rustok-cli seed` cutover.
- [x] Remove active `cargo loco` execution examples from server task/seed source comments; comments now point at the target `rustok-cli` shape while the legacy bridge remains in code.
- [x] Introduce a separate CLI crate/binary (`crates/rustok-cli`, bin `rustok-cli`) outside the server/Loco dependency graph; current implementation owns built-in help/list behavior, namespace-scoped discovery, `list --json` inventory output and consumes `rustok-cli-core`.
- [x] Introduce an initial explicit `rustok-cli::CommandRegistry` that aggregates command providers and rejects duplicate command keys; generated selected-distribution registry remains the next follow-up.
- [x] Introduce `rustok-cli-registry` as the selected distribution provider registry crate and connect the runner to it.
- [x] Introduce generated ops registry source that reads module `[provides.cli]` metadata and is checked by `node scripts/generate/generate-cli-registry.mjs --check`.
- [x] Introduce typed provider execution dispatch through `CommandProvider::execute` and `rustok-cli <namespace> <command>`.
- [x] Add `rustok-cli-platform` and select it through root `cli-registry.toml` so `rustok-cli core version` is provided through generated registry wiring, not hardcoded runner logic.
- [x] Add host-neutral `rustok-runtime::RuntimeComposition` and pass it into generated CLI provider factories for optional DB, settings and typed-handle composition without `apps/server` coupling.
- [x] Add standalone CLI runtime bootstrap from environment-provided settings and database URL, while preserving database-free commands.
- [x] Normalize CLI command arguments into `CommandRequest.args.options` and `CommandRequest.args.positionals` before provider execution.
- [x] Populate the generated registry with the first module-local command provider: `rustok-media-cli` provides `media cleanup` through module metadata and an explicit storage bootstrap from `RuntimeComposition` settings.
- [x] Move the first module-specific command to a module-local `cli/` adapter package, not to domain core and not to the central CLI crate.
- [~] Migrate `cleanup`, `rebuild`, `profiles_backfill`, `db_baseline`, and `create_oauth_app` to typed ops subcommands; `media_cleanup` is migrated to `rustok-cli media cleanup` and its Loco task/schedule are removed.
- [ ] Migrate seed profiles to `rustok-cli seed`.
- [ ] Migrate migration command wrappers to `rustok-cli migrate ...` over `Migrator`.
- [ ] Design follow-up for distribution-aware builds: module manifests, generated runtime/ops registries, external module packaging.
- [ ] Update docs/guides/scripts, remove `cargo loco task` from active instructions.

Exit gate: all maintenance flows run through `rustok-cli ...`; server binary does not depend on CLI crates; module commands are discovered through registry, not through a manual central dump; Loco tasks are not registered.

### Phase 4. Bootstrap, Initializers, Workers, Shutdown

- [ ] Dismantle `impl Hooks for App` into explicit bootstrap/lifecycle functions.
- [~] Replace Loco initializers with ordered bootstrap phases. The default-superadmin action now runs explicitly from `after_context` over `ServerRuntimeContext`; remaining host lifecycle phases still belong to the wider Hooks cutover.
- [ ] Migrate worker startup/shutdown to own lifecycle manager.
- [x] Introduce `crate::testing::get_server_app_context` as the local server test fixture bridge and move `app.rs`, `app_runtime` and `app_lifecycle` tests off direct `loco_rs::tests_cfg` imports; guardrails now forbid direct `loco_rs::tests_cfg` anywhere under `apps/server/src` except the bridge.
- [ ] Migrate tests from `loco_rs::tests_cfg` to `rustok-test-utils`.
- [ ] Remove dependency on `loco_rs::cli`.

Exit gate: server binary starts as a pure Axum runtime in targeted integration smoke.

### Phase 5. Last Adapters and Dependency Removal

- [ ] Remove `EmailProvider::Loco` or migrate it to a native provider without `ctx.mailer`.
- [ ] Remove `rustok-outbox/loco-adapter` and `rustok_outbox::loco`.
- [ ] Remove `loco-rs` from workspace dependencies and all package manifests.
- [ ] Update `Cargo.lock`.
- [ ] Archive Loco reference docs, scripts and CI freshness checks.

Exit gate: `rg "loco_rs|loco-rs|cargo loco|rustok_outbox::loco"` finds no active code/config paths; only archived docs with a link to this plan are allowed.

## Verification Gates

Minimum gates for each PR in this roadmap:

- `cargo fmt --check`
- targeted `cargo check` on affected crates
- `node scripts/verify/verify-api-surface-contract.mjs`
- `node scripts/verify/verify-loco-inventory.mjs`
- `cargo check -p rustok-server --no-default-features` for server/runtime changes
- `cargo xtask module validate <slug>` for affected modules

Final gate:

```bash
rg "loco_rs|loco-rs|cargo loco|rustok_outbox::loco" apps crates scripts Cargo.toml Cargo.lock
cargo check -p rustok-server --no-default-features
cargo check -p rustok-server
node scripts/verify/verify-api-surface-contract.mjs
```

In the final state, the first `rg` must return only archived/deprecated docs, if the search intentionally includes `docs/`.

## Documentation Tasks

- [x] Add this central plan to `docs/index.md`.
- [x] Update `apps/server/docs/README.md` so that Loco docs are historical context, not an active target.
- [x] Remove old `apps/server/docs/loco-core-integration-plan.md` integration roadmap.
- [x] Mark `apps/server/docs/LOCO_FEATURE_SUPPORT.md` as deprecated inventory.
- [x] Update `docs/AI_CONTEXT.md` after Phase 0 inventory so agents no longer follow old Loco rules.
- [x] Update `docs/ai/KNOWN_PITFALLS.md` after Axum/platform CLI guardrails appear.
- [x] Add ADR for Axum runtime and separate platform CLI cutover before Phase 1 code migration.

## Definition of Done

The plan is complete when:

1. `loco-rs` is absent from workspace dependencies and lockfile.
2. `apps/server` starts and tests as a pure Axum runtime without maintenance CLI code in the production binary.
3. All module-owned UI/server adapters use `rustok-api`/module contracts, not Loco context.
4. GraphQL, REST, Leptos `#[server]`, health, metrics, installer and separate maintenance CLI have verification evidence.
5. Old Loco documents are explicitly marked deprecated/archived and point to this plan.

## Principle for Selecting Loco Conventions

The migration does not mean blindly copying the Loco API or requiring difference from Loco everywhere. If an existing convention is already convenient for RusToK, matches our operational model and does not drag `loco_rs` as a runtime/dependency owner, it may be kept as a RusToK-owned contract.

Selection rule:

- preserve format, naming or workflow if they are useful for the project and become a documented RusToK contract;
- change or remove behavior if it exists only because Loco is structured that way;
- do not introduce compatibility for the sake of compatibility: after cutover, internal callers must depend on RusToK-owned contracts, even if the external appearance of these contracts resembles the former Loco format.
