# Documentation `apps/admin`

> **MANDATORY FOR AI AGENTS — Read these guides BEFORE any code changes:**
>
> **Module UI Package Guides (for `crates/rustok-*/admin` packages):**
> - [Architecture Guide](../../../docs/UI/module-package-architecture.md) — explains **FFA** (Fluid Frontend Architecture), `core/transport/ui` split, dual-path model
> - [Implementation Guide](../../../docs/UI/module-package-implementation.md) — **internal libraries** (`leptos-ui`, `leptos-ui-routing`, `rustok-graphql`, etc.), **i18n rules**, file structure, forbidden patterns
> - [Verification Guide](../../../docs/UI/module-package-verification.md) — verification commands, common errors
>
> **FSD Architecture:** This host follows **Feature-Sliced Design** layers: `app`, `widgets`, `features`, `entities`, `shared`. Module business UI must NOT be placed in `apps/admin/src/` — it belongs in owner module packages.
>
> **IMPORTANT RULES:**
> - **DO NOT write custom UI components** — check [Rust UI Component Catalog](../../../docs/UI/rust-ui-component-catalog.md) first
> - **DO NOT invent custom i18n** — use `rustok-ui-i18n-leptos` for Leptos module UI packages (see Implementation Guide)
> - **DO NOT remove GraphQL** when adding `#[server]` — both must coexist
> - **DO NOT write Leptos code in `core.rs`** — CI will reject it

Local documentation for the Leptos admin host application. This file captures only the live host-level contract; detailed plans, UI catalogs, and rollout notes are kept in separate documents.

## Purpose

`apps/admin` is the host/composition root for the RusToK administrative interface. The preferred product runtime for Leptos admin is SSR/hydrate in monolith deployment, with standalone CSR preserved as a debug/compatibility profile. The application:

- mounts host-owned screens and module-owned admin surfaces;
- holds a unified shell, navigation, RBAC-aware routing, and search entrypoint;
- uses `apps/server` as the backend surface for GraphQL, Leptos `#[server]`, and related runtime APIs.

`apps/admin` must not become the owner of module business logic. If a module provides its own admin UI, that surface remains alongside the module and is connected via a manifest-driven contract.

FFA classification: `apps/admin` is an `FFA-compatible composition host`, not a module-owned UI package. Its FFA responsibility is to maintain shell/routing/context composition and not move module-specific workflows from owner packages into the host.

The first host-level FFA slice has already been applied to app shell navigation: a portable sidebar policy
lives in `src/widgets/app_shell/core.rs` without Leptos dependencies, while `sidebar.rs` remains
a Leptos render/bind adapter. This split is enforced by a quick verifier
`npm run verify:frontend:host-ffa-contract`.

`/workflows` redirects to the owner-owned overview and templates surface at
`/modules/workflow`. The host still composes only the workflow detail editor, execution history,
and version history through `src/features/workflow/`; its native server-function adapter uses
`HostRuntimeContext`. The outstanding ownership transfer must move that remaining detail
surface atomically into `crates/rustok-workflow/admin/` and delete the host feature; no second
transport path is to be introduced.

The host-owned `/modules` control plane also receives only a narrow database snapshot from
`HostRuntimeContext`; `apps/admin` has no host-framework dependency.

The host-owned OAuth apps feature follows the same boundary: DTOs live in
`src/features/oauth_apps/model.rs`, list transport is selected through
`src/features/oauth_apps/transport/mod.rs`, GraphQL mutations stay behind
`transport/graphql_adapter.rs`, and the removed `src/features/oauth_apps/api.rs` facade must not be
restored.

The host-owned installer feature is a thin REST wizard over the server installer API. Installer DTOs
live in `src/features/installer/model.rs`, HTTP request code lives only in
`src/features/installer/transport/mod.rs`, and the page calls that transport facade instead of
holding raw REST wiring. The removed `src/features/installer/api.rs` facade must not be restored.

The host-owned cache health operator page keeps its read model in
`src/features/cache/model.rs` and selects native or GraphQL transport only through
`src/features/cache/transport/`. The page renders the transport result and does not own a raw
GraphQL request or server-function call.

The host-owned dashboard keeps its read DTOs, GraphQL selected path, SQL snapshots and native
server functions in `src/features/dashboard/`. Its page owns only host composition and rendering.

The host-owned email settings page keeps its settings DTOs and selected read/write transport in
`src/features/email/`; its page component owns form state only and does not issue raw GraphQL or
call a native adapter directly.

## Boundaries of Responsibility

`apps/admin` is responsible for:

- host routing, layout, navigation shell, and global UI capabilities;
- wiring module-owned admin pages through the generated registry;
- cross-module composition via separate host adapters and public owner contracts; `SearchAdminComposition` connects product-owned catalog option transport with search-owned UI props, passes `UiRouteContext.locale` and considers tenant module enablement without moving domain logic into the host;
- host-level locale propagation, auth/session UX, and permission-gated navigation;
- integration of host-owned operator scenarios that do not belong to a specific module.

`apps/admin` is not responsible for:

- moving module-specific CRUD and domain workflows into host code;
- its own locale negotiation chain inside module-owned packages;
- replacing the GraphQL transport just because a Leptos `#[server]` path exists.

## Runtime contract

- `apps/admin` supports three distinct runtime profiles that must not be mixed:
  `csr` for standalone Trunk/WASM, `hydrate` for the client half of SSR, and `ssr` for the server-side
  half/monolith.
- The preferred product path for Leptos admin is `ssr` + `hydrate` over `apps/server` as a same-origin backend. In this profile, native `#[server]` transport is the preferred internal data-layer.
- In the `csr` profile, the base transport must not require Leptos `#[server]`: GraphQL, auth, and REST go
  directly to `apps/server` via `/api/graphql`, `/api/auth/*`, and module-owned REST endpoints. Local
  `trunk serve` must proxy `/api/*` to `http://localhost:5150/api/*`. This profile is needed for debug/compatibility, not as a production default.
- In the `hydrate`/`ssr` and monolith profile, native `#[server]` endpoints `/api/fn/*` are considered available
  on the same backend origin and can be the preferred path for surfaces that need server-side runtime.
- If a surface supports a dual-path model, the GraphQL/REST selected path must actually work in `csr`;
  `#[server]` cannot be the only critical transport for standalone debug.
- GraphQL and the native Leptos `#[server]` path must coexist in parallel; `#[server]` does not replace `/api/graphql`.
- The reason for the split: monolith admin benefits from same-origin SSR/hydrate, server-side auth/session/policy, and a short Rust path via `#[server]`, but headless and standalone debug require a live GraphQL/REST fallback.
- The current data-layer for admin supports a dual-path model: the host selects native `#[server]` or GraphQL/REST by build/runtime profile, when that is provided for by the specific surface.
- `rustok-pricing/admin` is now one of these dual-path surfaces: the pricing package
  by default uses the native `#[server]` pricing runtime, leaving GraphQL
  fallback, and shows operator-side effective price context for
  `currency + optional region_id + optional price_list_id + optional quantity`,
  including a pricing-owned selector for active price lists, and also performs base-price
  variant updates via module-owned server-function transport.
- `apps/admin` is not considered a CSR-first host. CSR remains a mandatory standalone debug profile, but the architectural target for Leptos admin is an SSR-first host with headless GraphQL/REST parity.
- WebSocket transport `/api/graphql/ws` remains an active path for live update scenarios, including build/progress and subscription-based surfaces.
- Build/release native and GraphQL reads deserialize the same browser-safe
  `rustok-api` snapshots. The admin does not define a parallel build/release
  DTO or map owner persistence models.
- Host-owned `/install` is a Leptos wizard layer for the hybrid installer.
  It does not contain its own bootstrap logic: the screen collects an `InstallPlan`,
  calls `/api/install/preflight`, invokes `/api/install/apply`, polls
  `/api/install/jobs/{job_id}`, and shows persisted receipts from
  `/api/install/sessions/{session_id}/receipts`. The web layer works as a thin
  facade over `apps/server` and `rustok-installer`; full typed CLI install commands
  follow the shared executor-port extraction. This route is accessible before
  normal admin-auth, because the first install may not yet have a created
  superadmin; mutating install requests are protected by a setup-token guard on
  `/api/install/*`. The wizard does not prefill a sample admin password and admin
  PostgreSQL URL by default: production-like secret values must come
  via secret refs, and database creation is an explicit opt-in with a mandatory
  `pg_admin_url`.
- For `module-system` purposes, `/modules` is considered a closed repo-side operator surface: installation, removal, upgrade/deploy of modules and progress feedback are available from the Admin UI without a separate manual backend workflow.
- Host-owned `/modules` governance UI does not hold local policy heuristics: `registryLifecycle` remains a summary/read-model, but actor-agnostic `governanceActions` there are now limited to release-management hints (`owner-transfer`, `yank`), and the authoritative request-level contract for interactive governance is read by a separate bearer-auth fetch to `GET /v2/catalog/publish/{request_id}`; `reason` / `reason_code` and request-level availability are taken only from this status.
- `/modules` reads the typed registry audit payload only: `stage_key`, nested `owner_transition`, structured principal objects, and the current lifecycle/event read-side fields.
- For `apps/admin` this is considered the final repo-side contract: no new client-owned lifecycle is needed here going forward, only targeted verification mapping and periodic reconciliation of `/modules` UX with the server-driven policy surface.
- Toggle/install/uninstall/upgrade module composition must not have a local SSR SQL lifecycle duplicate: the host uses canonical server GraphQL/control-plane entrypoints, where CAS-update `platform_state` and build enqueue are atomic, and `manifest_ref`/`manifest_hash` are taken from the server-side snapshot contract.
- For module toggle, `apps/admin` maintains a GraphQL-only entrypoint contract (without a native fallback toggle path): error taxonomy, dependency/core checks, and journal semantics (`module_operations`) are defined by the server lifecycle service, not by local Leptos logic. The Leptos SSR adapter and UI must propagate `BAD_USER_INPUT`/`MODULE_HOOK_FAILED`/`INTERNAL_ERROR`, `correlation_id`, `requested_by`, `status`, `retryable_issue`, and related recovery fields without client-side remap.
- Module-control-plane GraphQL reads fail closed on transport or owner errors.
  The admin host does not synthesize registry, installation, tenant intent, or
  marketplace facts from its generated navigation registry. Native marketplace
  and registry lifecycle reads use the host-provided owner catalog and
  governance snapshot. The admin host contains no direct registry SQL,
  workspace/Cargo scanner, catalog hashing, dependency solving, or build
  planning path.

## Local debug launch

For local debugging without Docker, use `localhost` instead of `127.0.0.1`: on Windows, a loopback via `127.0.0.1`
may accept a TCP connection but not return an HTTP response. Working profile:

```powershell
# backend already listens on http://localhost:5150
$env:RUSTOK_MODULES_MANIFEST = (Resolve-Path ..\..\modules.local.toml)
$env:PATH="$env:USERPROFILE\.rustok\tools\trunk;$env:PATH"
trunk serve --address ::1 --port 3001
```

For this profile, the backend starts with `modules.local.toml`, where embedded admin/storefront are disabled. The root
`modules.toml` describes monolith/release composition and requires `embed-admin`; in the current Windows debug environment
the SSR build of embedded `apps/admin` crashes due to memory (`rustc-LLVM ERROR: out of memory`), so local debug
splits the backend and the external Trunk host.

`apps/admin` in standalone debug runs as a CSR host, so Trunk must build the binary artifact
`rustok-admin`, not the library artifact `rustok_admin`. This is specified in `index.html` via
`data-target-name="rustok-admin-app"`: the binary runs `main()` and mounts the shell in `body`.

Tailwind CSS for this debug profile is built by the Trunk post-build hook `scripts\tailwind-build.cmd`.
The hook writes `output.css` to `TRUNK_STAGING_DIR`, so CSS survives the `dist` cleanup inside the Trunk pipeline.
Locally, the command can be run separately just for a quick CSS check:

```powershell
npm.cmd install
npm.cmd run tw:build
```

`apps/admin/input.css` uses Tailwind v4 `@import "tailwindcss"` and explicit `@source` entries. `tailwind.config.js`
must include `apps/admin/src`, shared Leptos UI crates and module-owned admin UI packages
`crates/**/admin/src/**/*.rs`. If `dist/output.css` is missing or the source globs do not cover module UI packages,
the shell will load partially or without styles. This does not change the production target: the architectural path for Leptos admin remains
SSR/hydrate over `apps/server`, and CSR is needed for standalone debug and testing module-owned UI packages.

Leptos admin must not visually diverge from Next admin as a separate product. Auth shell, navigation shell,
route-selection UX, and module-owned UI containers must follow the common admin UI contract. Next admin remains
a parallel React/Next host, while Leptos admin is the canonical operator surface for the SSR/monolith path; any
discrepancies found are filed as parity debt and fixed on a case-by-case basis.

## Contract for module-owned admin UI

- Source of truth for connecting UI modules: `modules.toml` plus `rustok-module.toml`.
- `apps/admin/build.rs` reads the manifest layer and generates wiring into `OUT_DIR`.
- A publishable Leptos admin surface must declare `[provides.admin_ui].leptos_crate`; the presence of `admin/Cargo.toml` alone is not considered integration.
- The host mounts module-owned pages via `/modules/:module_slug` and nested variant `/modules/:module_slug/*module_path`.
- The sidebar is built from manifest-driven navigation metadata. `[provides.admin_ui].nav_group` and `nav_order` are optional overrides; if they are not set, the host groups first-party modules into standard buckets `Content`, `Commerce`, `Runtime`, `Governance`, `Automation`, `Other`.
- The canonical source for module submenu is `[[provides.admin_ui.child_pages]]`.
- Each module-owned admin surface gets a root `Overview` item; declared child pages become nested links under the module container. The host hides disabled tenant modules and empty containers.
- Tenant/module settings remain in the host-owned `/modules` governance UI. If `rustok-module.toml` contains `[settings]`, the sidebar adds a contextual link `/modules?module_slug=<slug>`; module-owned packages do not duplicate this editor.
- Recovery for failed module lifecycle post-hook operations remains a host/control-plane scenario: Leptos admin shows a host-owned `Lifecycle recovery` block, reads `failedModuleOperationRecoveryPlans`, and calls `retryFailedModuleOperationPostHook` / `compensateFailedModuleOperation` via canonical GraphQL helpers in `features/modules/transport`; local SQL, local rollback, and custom lifecycle taxonomy are prohibited.
- The host passes the effective locale via `UiRouteContext.locale`; module-owned Leptos packages must use this value and must not introduce their own query/header/cookie fallback chain.
- Module-owned admin packages must support the same runtime split: `#[server]` preferred in SSR/hydrate, GraphQL/REST fallback for standalone CSR/debug. The package must become neither GraphQL-only for monolith nor `#[server]`-only for headless/debug.
- Core modules with UI are subject to the same ownership rule as optional modules: the presence of UI does not make the host the owner of the module surface.
- The capability-owned MCP surface is connected via the host route `/mcp`, which mounts only `rustok_mcp_admin::McpAdmin`; persisted scaffold writes and transport logic remain in the owner port/server provider, not in `apps/admin`.
- The route-selection contract is also host-owned: `apps/admin` sanitizes the query against a typed schema from
  `rustok-api`, gives module packages an already canonical route context, and provides generic
  Leptos query plumbing via `leptos-ui-routing`.
- `rustok-seo-admin` after cutover no longer holds entity selection/state at all: the `seo` route
  uses only `tab` for control-plane navigation, and page/product/blog/forum SEO authoring
  lives in owner-module packages.
- The same `rustok-seo-admin` holds route/query orchestration in a shell component, and renders bulk/redirects and
  sitemaps/robots/defaults/diagnostics via separate section components inside the package,
  without moving this UI split into the host.
- Canonical ownership is separately established: entity SEO authoring must live in owner-module
  admin packages (`pages`, `product`, `blog`, `forum`), while `rustok-seo-admin` after cutover remains
  only a cross-cutting SEO infrastructure surface.
- This cutover has already begun in code: `rustok-pages/admin`, `rustok-product/admin`, and `rustok-blog/admin`
  embed owner-side SEO panels via `rustok-seo-admin-support`, while `rustok-forum/admin`
  holds a capability slot until forum targets appear in the shared runtime.
- For module-owned admin pages, selection state lives only in the URL; absence of a valid key leads to an
  empty state, and an invalid/missing entity must not leave stale detail/form state.

## Interactions

- With [`apps/server` documentation](../../server/docs/README.md): backend runtime, GraphQL, `#[server]`, auth/session, registry, and health surfaces.
- With [hybrid installer ADR](../../../DECISIONS/2026-04-26-hybrid-installer-architecture.md): installer-core, canonical CLI, HTTP adapter, and thin Leptos wizard layering.
- With [manifest layer contract](../../../docs/modules/manifest.md): module registration, UI ownership, and settings schema.
- With [module and application registry](../../../docs/modules/registry.md): map of platform modules, support crates, and host applications.
- With module-owned admin packages: the host knows only the registration contract, route context, and secondary nav metadata; internal sub-routing and domain UI remain inside the package.

## Verification

Minimal local path for changing `apps/admin`:

- `cargo xtask module validate <slug>` for modules whose admin surfaces are affected;
- targeted `cargo check` or `cargo test` for affected Leptos crates;
- `npm run verify:i18n:ui` and related contract checks, if locale bundles or host-provided translations are affected;
- targeted check of host routing and permission-aware navigation for affected screens.

## Related documents

- [Implementation plan](./implementation-plan.md)
- [Manifest layer contracts](../../../docs/modules/manifest.md)
- [Module and application registry](../../../docs/modules/registry.md)
- [Rust UI component catalog](../../../docs/UI/rust-ui-component-catalog.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Documentation map](../../../docs/index.md)
