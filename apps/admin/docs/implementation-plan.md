# Admin App Implementation Plan

## Current Contract

`apps/admin` is an FFA-compatible Leptos composition host. It owns shell, routing,
host-level operator screens and cross-module composition, while module business UI
belongs in owner packages under `crates/modules/rustok-*/admin`.

The live host structure is:

- `app/` for app wiring, generated module registry and providers;
- `widgets/` for host shell widgets with portable policy in `core.rs`;
- `features/` for host-owned operator features and cross-module composition;
- `entities/` for host-local read models;
- `shared/` for host shared transport, UI and context helpers.

Host-owned features use explicit model/transport boundaries:

- `features/workflow/model.rs` and `features/workflow/transport/`;
- `features/oauth_apps/model.rs` and `features/oauth_apps/transport/`;
- `features/installer/model.rs` and `features/installer/transport/`;
- `features/cache/model.rs` and `features/cache/transport/`;
- `features/dashboard/model.rs` and `features/dashboard/transport/`;
- `features/email/model.rs` and `features/email/transport/`;
- `features/modules/transport/` for module control-plane transport.

Pages and components call public transport facades only. Raw GraphQL, REST and
native `#[server]` functions stay behind those transport boundaries.

Module-control-plane GraphQL adapters now propagate transport/owner failures
instead of constructing successful-looking registry, installation, tenant, or
marketplace responses from compile-time navigation metadata. This removes a
second state model while preserving the native and GraphQL transport paths.
Native marketplace and registry lifecycle reads consume the host-provided
`SharedModuleMarketplaceCatalog`, whose public list and detail view is
`rustok_api::MarketplaceModule`. The server converts its owner-private
`ModuleMarketplaceEntry` once, reduces registry-principal JSON to display-label
scalars, and gives GraphQL and native the same canonical facts. Admin aliases
the shared DTO family; it does not parse principal JSON, define fallback
lifecycle types, or supply transport-specific defaults. The former direct
registry SQL, workspace/Cargo scanning, catalog synthesis, canonical hashing,
dependency solving, and build planning have been deleted from the admin host.
Registry publish-status and generic governance mutation responses likewise use
the strict shared `rustok_api::RegistryPublishStatus` and
`RegistryMutationResult` types. The native adapter consumes the complete
owner-issued validation-stage contract directly, without a local status model,
defaulted execution-policy fields, or a response-schema fallback.
Lifecycle events also carry the canonical typed
`RegistryAutomatedCheckLifecycle` collection. The GraphQL selection includes
`automatedChecks`, and the detail panel displays the newest owner-issued check
set with its optional detail rather than parsing raw event JSON or creating a
local empty result.
The module operator surface also consumes per-registry freshness through the
same owner catalog facade. Both native and GraphQL paths require
`modules.manage`; the UI renders logical registry identity, status, last
success, and consecutive failures without learning endpoint or remote error
details.

Effective module availability is an owner-issued
`rustok_api::ModuleEffectivePolicyView`, not a host calculation over tenant
rows. The native `module_effective_policy_native` function receives the
host-composed, active-composition reader and GraphQL exposes the matching
`moduleEffectivePolicy` query. `EnabledModulesProvider` derives its enabled
slugs from that view, and the module registry refreshes its owner-issued state
after lifecycle commands rather than optimistically mutating a local enabled
set. Core identity does not bypass a channel, maintenance, or other unavailable
owner decision.

Installed static-module reads use `rustok_api::StaticInstalledModuleView`. The
server injects `SharedStaticInstalledModuleReader` into the host runtime;
GraphQL maps the same projection and the Admin entity aliases it. Native Admin
does not decode the active manifest or expose source-control and filesystem
locators.

Static tenant-lifecycle views use the host-composed
`SharedStaticModuleLifecycleReader`. Native Admin receives the same
active-composition-aware owner projections as GraphQL and never deserializes
manifest defaults to rebuild lifecycle state.

The full static module registry uses
`rustok_api::StaticModuleRegistryView` through
`SharedStaticModuleRegistryReader`. The server resolves one active composition
and applies catalog metadata, effective policy, lifecycle revisions, and the
host-provided locale before either GraphQL or native Admin consumes the view.
The Admin entity aliases that DTO; it does not generate manifest metadata,
rebuild lifecycle/policy state, omit UI flags, or invent defaults.

Static composition install, uninstall, and upgrade use GraphQL only. The host
reads the owner-issued composition revision, passes it as a required optimistic
precondition with a fresh UUID idempotency key, and relies on authenticated
server context for tenant, actor, and permission. It has no local manifest,
hash, build-plan, retry, or fallback implementation; terminal owner receipt
replay returns the original build after a later composition change.

Static lifecycle enablement uses the same GraphQL-only ownership boundary. The
host sends a fresh UUID idempotency key but never supplies tenant, actor,
permission, correlation, or `requested_by` text; the server derives those facts
from authentication and the owner journals exact replay, including an explicit
no-op intent. Static lifecycle revision CAS across enablement and settings is
still unfinished and must not be represented as composition-revision parity.
Post-hook retry and compensation use the same typed GraphQL boundary with a
fresh UUID idempotency key per user action; the host does not provide actor
display text or lifecycle correlation.

## Active Work

- Keep host FFA guardrails current in `scripts/verify/verify-frontend-host-ffa-contract.mjs`.
- Keep `apps/admin/docs/README.md` synchronized with host-owned feature boundaries.
- Keep module-owned UI out of `apps/admin/src/features/`, except host composition and
  platform operator surfaces.
- Keep GraphQL and native `#[server]` paths in parallel where a surface is
  public/headless-capable.
- Keep locale propagation host-owned; module UI receives effective locale from
  host context and must not add local cookie/header/query fallback chains.
- Keep module-control-plane native reads behind owner services and the
  host-composed marketplace catalog, static-installed-module, and
  static-module-registry handles.
- Continue the owner-by-owner
  [Richtext cutover](../../../docs/modules/rich-text-implementation-plan.md).
  Blog now mounts the shared sandboxed editor during hydration and selects
  native `#[server]` for SSR/hydrate with parallel GraphQL for CSR/headless use.
  Remaining owners must reuse that capability and must not retry failed
  mutations through another protocol.

## Open Improvement Areas

- Add route-level and action-level permission checks where a host screen still
  relies only on backend rejection.
- Add UX flow metrics for critical admin actions, failures and latency.
- Propagate correlation ids through host transport helpers where backend surfaces
  expose them.
- Expand focused component and contract tests for host operator features.
- Keep Leptos admin and Next admin behavior aligned for loading, empty, error and
  permission-gated states.
- Prove richtext frame CSP, host-provided i18n/locale, accessibility,
  save/reload, and server-rendered read parity without weakening the parent
  `style-src-attr 'none'` policy.

## Verification

For host FFA changes, run:

```powershell
cargo fmt --manifest-path apps\admin\Cargo.toml --check
cargo check --manifest-path apps\admin\Cargo.toml --lib -j 1
node scripts\verify\verify-frontend-host-ffa-contract.mjs
node scripts\verify\verify-module-control-plane-write-path.mjs
node scripts\verify\verify-workflow-admin-boundary.mjs
git diff --check
```

When touching module-owned packages mounted by this host, also run the relevant
module verifier and `cargo xtask module validate <slug>`.
