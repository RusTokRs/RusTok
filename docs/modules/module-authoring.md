---
id: doc://docs/modules/module-authoring.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# How to Write a Module in RusToK

This document is the canonical entry point for a developer or AI agent writing a new module or making a major module refactor. It does not duplicate architecture documents line by line, but fixes the practical contract: how to assemble the backend and UI of a module so as not to break platform boundaries.

If you need a short answer to the question "where to start", the order is:

1. Define the module's ownership and runtime role.
2. Assemble the backend according to the platform contract.
3. Only then add UI as a module-owned package.
4. Update local module docs and central docs.

## Before Starting

Before any module changes, be sure to check:

- [Module Platform Overview](./overview.md)
- [Module Architecture](../architecture/modules.md)
- [`modules.toml` and `rustok-module.toml` Contract](./manifest.md)
- [Platform Database Schema](../architecture/database.md)
- [i18n Architecture](../architecture/i18n.md)
- [API Architecture](../architecture/api.md)
- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Backend Module Verification Guide](../backend/module-backend-verification.md)
- [`apps/server` Documentation](../../apps/server/docs/README.md)
- [`apps/admin` Documentation](../../apps/admin/docs/README.md)
- [Workspace CLI `xtask`](../../xtask/README.md)
- [Implementation Plans Registry](./implementation-plans-registry.md)

## What Counts as a Module

In RusToK, a module is not "any crate in `crates/`". A canonical platform module:

- is declared in `modules.toml`;
- has a `slug`, ownership and runtime contract;
- passes scoped validation via `cargo xtask module validate <slug>`;
- publishes backend and, if necessary, module-owned UI surfaces through the manifest.

A support/crate/capability layer can live alongside a module, but that does not automatically make it a tenant-toggled module. This is especially important for `rustok-core`, `rustok-api`, `rustok-storage`, `rustok-mcp`, `rustok-ai`, `alloy`, `flex` and similar foundation layers.

If a support/capability crate publishes a runtime seam, the canonical connection method is now one:

- module-owned backend crate registers capability via `RusToKModule::register_runtime_extensions(...)`;
- host builds a single `ModuleRuntimeExtensions` and passes it to all shared entrypoints;
- consumer entrypoints that depend on such a capability must fail explicitly when the shared registry is absent, rather than silently falling back to hardcoded built-ins; the error message must be actionable (which capability was not found, which consumer entrypoint is affected, which module/owner is expected and how to fix the configuration). Graceful degradation is only allowed as an explicitly documented opt-in mode (e.g. feature-disabled/read-only), with a warning in logs/metrics and without implicit substitution of built-ins;
- if a capability is introspected by shared operator/admin surfaces, the provider must publish owner-aware metadata instead of forcing the host to map slugs to labels rigidly;
- a capability crate does not automatically get its own slug in `modules.toml` as a result.

For SEO-capable modules, an additional rule applies:

- provider in `rustok-seo-targets` only returns typed target records and a safe `template_fields` map (`title`, `description`, `route`, `locale`, slug/handle/id fields);
- templates for `title`, `meta_description`, canonical, robots, Open Graph and Twitter are rendered only by `rustok-seo`;
- the owner module must not introduce its own SEO-template runtime or pass raw HTML/JSON into the template context.

If a target participates in bulk SEO, the provider must provide stable summaries and fields sufficient for safe remediation: `preview_only`, `apply_missing_only`, `overwrite_generated_only` and `force_overwrite_explicit` are executed in `rustok-seo`, not in the owner module.

## FFA/FBA-First Gate for New Modules

A new module or major module split must not start with a host-owned UI, ad-hoc transport
handler or direct addition of tables to an umbrella module. Before the first transport/UI PR,
an FFA/FBA gate is mandatory:

1. Fix the `slug`, ownership, runtime role and local `docs/implementation-plan.md` with an
   FFA/FBA status block.
2. Describe the canonical domain/application service contract before REST, GraphQL, `#[server]` or
   host wiring.
3. Describe typed request context for tenant/auth/locale/channel/policy/trace data and stable
   error mapping between domain errors and transport errors.
4. Describe data ownership, consistency model, migrations and i18n storage contract.
5. Express cross-module dependencies through explicit ports/events/provider seams, not through
   access to other repositories' internals or host-specific globals.
6. Add a row to the central FFA/FBA readiness board before module-owned UI appears, and if
   there is no UI, leave the surface as `no module-owned UI` / `no_ui_boundary` with FBA status.
7. Only after this add transport adapters (`#[server]`, GraphQL, REST/RPC) and
   module-owned UI as a thin adapter through the module-owned `transport/` facade.

If an already completed functional slice does not pass this gate, the next change set first
brings it to FBA-ready boundary evidence and only then extends functionality.

### Structural Minimum for an FBA Increment

For FBA translation, the same standard applies as in the unified plan
`docs/research/fluid-backend-architecture-unified-plan.md`: local status block, central board,
runtime metadata/registry, owner-owned contracts, anti-drift/fallback verification and evidence
packet. The target crate structure is described in section `2.3` of the unified plan: `*-grpc` crate and
repository interfaces are optional late-stage seams, not mandatory scaffolding for the first PR.
The working name FBA is not carried over into type names without necessity: code-facing
contracts use neutral `*Port`, `PortContext`, `PortError`, `provider` and `consumer`.

### New Implementation Without Legacy Layer

RusToK is at the stage of initial implementation. FFA/FBA refactoring here is not
a migration of an old production system and should not preserve the former internal architecture.

Mandatory rules:

1. Immediately implement the target contract and target module structure.
2. In one change set, migrate all internal call sites to the new port, adapter or entry point.
3. After migration, delete replaced ports, facades, adapters, aliases, feature flags and wiring.
4. Do not add compatibility wrappers, dual old/new paths, fallback to legacy implementation,
   deprecated aliases or temporary old ports "just in case".
5. Do not keep the old signature just for internal callers: callers must be
   rewritten to the target contract.
6. A temporary bridge is only allowed by direct requirement for staged external migration.
   It requires a removal owner and a specific removal deadline in the module plan.

Mandatory current platform contracts are not considered legacy compatibility. For example,
GraphQL remains a parallel contract according to platform rules, but must not serve as a
fallback to an outdated internal port or duplicate old business logic.

## Backend

The detailed backend contract lives in the backend module guides:

- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Backend Module Verification Guide](../backend/module-backend-verification.md)

These guides are mandatory for module services, ports, GraphQL/REST handlers,
Leptos `#[server]` adapters, FBA metadata and CLI adapters. The rest of this section
is a summary and must not be used to bypass the detailed backend guides.

### 1. First Fix the Runtime Contract

Minimum for a backend module:

- entry in `modules.toml`;
- `rustok-module.toml` with correct `module.slug`, `module.version`, `module.description`, `module.ui_classification`;
- root `README.md` in English;
- local `docs/README.md` and `docs/implementation-plan.md` in English.
- for a new module/support crate, be sure to add a row to the [implementation plans registry](./implementation-plans-registry.md) (`Global board`) according to the registry format: minimum `Plan ID`, `Module/Crate`, `Plan doc` and `Status`.

Canon:

- composition and module taxonomy: [overview.md](./overview.md)
- manifest contract: [manifest.md](./manifest.md)
- ownership map: [registry.md](./registry.md)

### 2. Do Not Invent Your Own Backend Contract

The module backend must fit into the general platform flow:

- transport ownership goes through `apps/server`, but the business/domain contract remains with the module;
- Leptos `#[server]` — default internal data layer for Leptos surfaces, but GraphQL remains in parallel;
- REST is only needed where an explicit HTTP contract is really needed: integrations, webhooks, ops, module-owned routes;
- you cannot make package-local auth, locale, tenant or RBAC shortcuts.
- runtime registries and provider seams must be registered through the common `ModuleRuntimeExtensions`,
  not through host-specific globals or ad-hoc singleton wiring.

Canon:

- API surfaces: [api.md](../architecture/api.md)
- routing and transport boundaries: [routing.md](../architecture/routing.md)
- server host contract: [apps/server/docs/README.md](../../apps/server/docs/README.md)
- backend implementation guide: [module-backend-implementation.md](../backend/module-backend-implementation.md)

Foundation ownership is fixed:

- stable cross-boundary contracts stay in `rustok-api`;
- executable runtime helpers go to `rustok-runtime`;
- Axum response/error/extractor helpers go to `rustok-web`;
- FBA provider/consumer metadata goes to `rustok-fba`;
- CLI command/provider contracts, including typed command execution, go to
  `rustok-cli-core`.

Do not expand `rustok-api` or `apps/server` just because a backend helper is needed.

The module backend layout is fixed as well:

- domain/application code goes in `crates/rustok-<module>/src`;
- public contract and FBA evidence artifacts go in module-local `contracts/`;
- local roadmap and FFA/FBA evidence go in module-local `docs/`;
- operational command adapters go in module-local `cli/` and depend on
  `rustok-cli-core`;
- `admin/` and `storefront/` remain optional module-owned UI adapter packages;
- `apps/server` only mounts routes, assembles runtime state and composes owner-owned
  entrypoints.

Do not put CLI adapters into the domain crate, do not put module business logic into
`apps/server`, and do not collect every third-party module command in one central crate. The
future platform CLI aggregates module-local providers through an explicit registry.

### 3. Data and Migrations Follow the Common Storage Contract

You cannot invent your own storage schema for text, locale and identity.

Basic rules:

- language-agnostic state lives in base tables;
- short localizable fields live in `*_translations`;
- heavy localizable content lives in `*_bodies` if necessary;
- `locale` is stored normalized;
- audit payload and technical metadata must not turn into business copies;
- module-owned migrations are exported through a local `migrations()` and the `MigrationSource` trait; if a migration creates FK or other strict ordering against another module crate's tables, there must be a `migration_dependencies()` with `MigrationDependencyDescriptor`, and the module `MigrationSource::migration_dependencies()` must return this exporter; `rustok-migrations` aggregates descriptors through `MigrationSource` for all module crates whose migrations are included in the platform migrator;
- a descriptor must only reference real migration names and pass server migrator tests for missing dependency, duplicate descriptor and cycle.

Canon:

- DB contract: [database.md](../architecture/database.md)
- i18n contract: [i18n.md](../architecture/i18n.md)

### 4. Do Not Put Platform Rules in Strings and Ad-hoc JSON

For a backend module it is forbidden:

- to authorize actions by untrusted strings or header-based actor model;
- to build live authority from display labels;
- to keep the canonical read contract in arbitrary `details` JSON when a typed schema already exists;
- to mix public contract and internal audit storage.
- to hide module-owned runtime capability registration inside the host app such that a new provider
  requires manual editing of a central feature module instead of `register_runtime_extensions(...)`.

If an actor/principal/read-model is needed, use a typed contract, not string heuristics.

### 5. Backend Verification

Minimum checklist before completing work:

1. `cargo xtask module validate <slug>`
2. `cargo check -p rustok-server --lib`
3. targeted `cargo test` for the module and affected host/runtime
4. updated local module docs
5. updated central docs if architecture/runtime contract changed
6. module plan added/synchronized in `docs/modules/implementation-plans-registry.md`

## UI

### 1. UI in RusToK is Module-Owned, Not Host-Owned by Default

If a module publishes UI, that UI must live next to the module:

- Leptos admin/storefront — through `admin/` and `storefront/` sub-crates;
- Next.js surfaces — through corresponding host packages, but the ownership UI contract still remains with the module;
- the host only mounts these surfaces and provides route/auth/locale/runtime context.

Canon:

- module composition: [modules.md](../architecture/modules.md)
- UI package map: [UI_PACKAGES_INDEX.md](./UI_PACKAGES_INDEX.md)
- quickstart for UI packages: [UI_PACKAGES_QUICKSTART.md](./UI_PACKAGES_QUICKSTART.md)

### 2. For Leptos UI, `#[server]` First, Then Everything Else

For module-owned Leptos UI, the mandatory rule applies:

- the internal data layer is built by default through native `#[server]` functions;
- GraphQL remains a parallel transport contract and is not removed;
- you cannot replace existing GraphQL just because a `#[server]` path has appeared.

Canon:

- UI/GraphQL/server-functions: [graphql-architecture.md](../UI/graphql-architecture.md)
- admin host contract: [apps/admin/docs/README.md](../../apps/admin/docs/README.md)

### 3. UI Does Not Choose Locale Itself

A module-owned UI package does not have the right to invent its own locale chain.

Rule:

- effective locale comes from the host/runtime;
- Leptos packages read host-provided `UiRouteContext.locale`;
- Next packages use host/runtime locale providers;
- query/header/cookie fallback chain at the package level is forbidden.

Canon:

- i18n contract: [i18n.md](../architecture/i18n.md)
- UI host contract: [apps/admin/docs/README.md](../../apps/admin/docs/README.md)

### 4. UI Wiring Goes Through Manifest, Not Through "Magical Crate Presence"

The mere existence of an `admin/` or `storefront/` directory does not mean the surface is integrated correctly. The canonical source of truth here is the manifest.

You need to:

- declare the UI surface in `rustok-module.toml`;
- keep `module.ui_classification` in line with actual wiring;
- not leave orphaned host dependencies or feature entries after refactoring.

Canon:

- manifest/UI wiring: [manifest.md](./manifest.md)
- module registry/index: [registry.md](./registry.md), [_index.md](./_index.md)

### 5. Migration Dependencies and Descriptor Evidence

If module migrations have cross-module FK/order assumptions, the module must
declare these dependencies alongside its migrations through `migration_dependencies()`
in the module migration source implementation. This does not replace `depends_on` from `modules.toml`:

- `depends_on` describes the runtime/module graph;
- `migration_dependencies()` describes ordering constraints between specific migrations.

Rules for new migrations:

1. If a migration references a table, index, enum/type or seed state of another module,
   add a descriptor dependency on the specific upstream migration.
2. The descriptor must reference only a real existing migration name/id.
3. The server migrator must aggregate descriptors through module `MigrationSource`, not
   through a package-local allowlist of a single crate.
4. Duplicate, missing descriptor and cycle failures are considered a migration contract error, not
   a flaky test.
5. For PostgreSQL smoke, use apply-from-zero and, for critical changes,
   incremental mode.

Minimum checks:

```bash
./scripts/verify/verify-migration-smoke.sh
RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 ./scripts/verify/verify-migration-smoke.sh
```

For failure diagnostics, see [runtime guardrails runbook](../guides/runtime-guardrails.md#wave-6-diagnostics-runbook).

### 6. UI Verification

Minimum checklist:

1. `cargo xtask module validate <slug>`
2. targeted `cargo check` for the UI crate and host app
3. `npm run verify:i18n:ui`, if locale bundles or locale wiring are affected
4. UI package docs and host docs updated if surface contract changed

### 7. Mandatory FFA/FBA Status Block for Modules with UI

For each module-owned UI package (admin/storefront/host-integrated surface) in the local
`docs/implementation-plan.md`, a status block is mandatory:

```md
## FFA/FBA status

- FFA status: `not_started | in_progress | phase_b_ready | parity_verified`
- FBA status: `not_started | in_progress | boundary_ready | transport_verified`
- Evidence:
  - UI/core/transport decomposition status
  - native `#[server]` + GraphQL parity status
  - backend boundary status (in-process/remote-ready), if applicable
- Last verified at (UTC):
- Owner:
```

Rules:

1. If the UI contract, transport wiring or module boundary is modified, the status block is updated in the same PR.
2. If a local block status changes, the central entry in `docs/modules/registry.md` (FFA/FBA readiness board section) is synchronously updated.
3. You cannot set `parity_verified`/`transport_verified` without explicit verification evidence in the PR and in the local plan.

### 8. Rule for Modules Whose UI is Planned but Not Yet Implemented

To avoid losing control over future UI surfaces, for modules with planned UI, a
mandatory preliminary rule applies:

1. If the UI is not yet implemented, the local `docs/implementation-plan.md` must still
   have an `## FFA/FBA status` block with `not_started` statuses and an explicit note in `Evidence`
   that the UI surface is planned but not published.
2. In the central `docs/modules/registry.md` (FFA/FBA readiness board), such a module must
   have a row with `not_started` status and a correct `Source plan`.
3. In the PR where module-owned UI (admin/storefront/host-integrated) first appears,
   the implementer must in the same change:
   - update the local status to at least `in_progress`;
   - synchronize the corresponding row in the central board;
   - attach initial verification evidence (minimum validate/check + transport parity note).

### 9. Periodic Release Verification Handoff

When a module is visited by the cyclic pre-release sweep in
[`docs/verification/PLATFORM_VERIFICATION_PLAN.md`](../verification/PLATFORM_VERIFICATION_PLAN.md),
update one current handoff block in the module's existing
`docs/implementation-plan.md`:

```md
## Periodic release verification handoff

- Cycle: `cycle-NNN`
- Status: `pending | in_progress | completed | blocked`
- Last verified at (UTC):
- Scope inspected:
- Findings: `P0=0, P1=0, P2=0, P3=0`
- Fixed in this pass:
- Remaining risks or blockers:
- Evidence:
- Next action:
- Resume command:
```

The master plan owns the current cursor and resettable queue. The local block owns the
component-specific handoff and is replaced on the next visit rather than accumulated
as a historical log. Completion counts only when the local cycle identifier matches
the current master cycle. Unfixed work must also be reflected in the module plan's
current priorities; a chat message or terminal transcript is not durable state.

## What is Forbidden

When writing a module, you cannot:

- consider any crate a module without `modules.toml`;
- invent a package-local i18n contract;
- transfer module-owned domain/UI ownership to the host app without an explicit reason;
- make runtime authority from string actors, display labels or untrusted headers;
- store localizable business text directly in base rows if the module already follows a multilingual contract;
- replace a typed public contract with raw `details` JSON;
- keep old internal ports, adapters, facades, aliases or execution paths after
  introducing the target contract;
- add a compatibility/fallback-to-legacy layer without a direct requirement, removal owner
  and removal deadline in the module plan;
- update only code without local/central docs if the contract has changed.

## Quick Decision Template

If an agent or developer needs to make a quick decision, use this order:

1. Is this a platform module or support/capability crate? (see [overview.md](./overview.md), [modules architecture](../architecture/modules.md))
2. What is its backend contract: GraphQL, REST, `#[server]`, events, migrations? (see [manifest contract](./manifest.md))
3. What data is language-agnostic and what is localized? (see [database schema](../architecture/database.md))
4. Does the module have a module-owned UI surface? (see [overview.md](./overview.md))
5. How does the host provide it with auth, locale, routing and tenant context? (see [modules architecture](../architecture/modules.md))
6. Which docs and verification gates must change together with the code? (see [PR / Review Checklist](#pr--review-checklist))

## PR / Review Checklist

This checklist is needed for any new module, major module refactor or module contract change. It can be run before a PR or during review.

### Backend checklist

1. The module is actually declared in `modules.toml`, not just exists as a crate.
2. `rustok-module.toml` is synchronized with the actual runtime contract.
3. `module.slug`, `module.version`, `module.description` and `module.ui_classification` do not diverge from code and wiring.
4. The backend does not invent its own auth, tenant, locale or RBAC contract.
5. Leptos `#[server]` is added as an internal data layer where needed, but GraphQL is not removed or secretly replaced.
6. REST is added only where an explicit HTTP contract is truly needed.
7. Language-agnostic state is stored in base tables, localizable fields are placed in `*_translations` or `*_bodies`.
8. Typed public contract is not replaced by string heuristics, `details` JSON or header-based authority.
9. Migrations, read-model and transport are updated consistently, without a half-migrated contract.
10. Old internal ports, adapters, facades, aliases and call sites are removed; dual old/new path is absent.
11. `cargo xtask module validate <slug>` and targeted `cargo check` / `cargo test` pass.

### UI checklist

1. UI surface remains module-owned, not spread into the host app without an explicit reason.
2. UI wiring is described in the manifest, not based on "magical" crate or route presence.
3. `module.ui_classification` matches actual admin/storefront surfaces.
4. Leptos UI uses the native `#[server]` path as the default internal data layer.
5. GraphQL transport remains in parallel if the module already publishes a GraphQL surface.
6. The UI package does not invent its own locale selection and consumes the host-provided effective locale.
7. The host gives the module only context and mounting, not ownership of the domain UI contract.
8. No orphaned host dependencies, feature flags or outdated wiring after refactoring.
9. Local UI package docs and host docs are updated if the surface contract changed.
10. Targeted UI checks and `verify:i18n:ui` pass if locale bundles or locale wiring are affected.

### Docs checklist

1. Component root `README.md` updated.
2. Local `docs/README.md` updated.
3. Local `docs/implementation-plan.md` updated if the roadmap or target state changed.
4. `docs/index.md` updated if the documentation map changed.
5. No duplicate new document if a suitable one already existed.

## Scripts placement policy

- Module-specific scripts must live near the module in `crates/<module>/scripts/` (or `apps/<app>/scripts/` for app-owned scripts).
- Repository-level `scripts/` is reserved for cross-platform orchestration and multi-module runners.
- If a script affects module runtime/public contracts, update both local module docs and central `docs/` references in the same change.
