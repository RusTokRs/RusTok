# Implementation plan for `rustok-channel`

Status: experimental core capability; `v0 baseline complete`. Current focus is
post-v0 rollout policy lifecycle, runtime integration parity, no-compile executable FBA fallback evidence and locking down the decision on built-in host fast-path.

## Current state

- Plan is synchronized with the current policy lifecycle implementation: update/reorder/disable for rules already present in domain/service and server transport.
- Rollout decision is locked: built-in host fast-path remains a separate fast layer between explicit selectors and typed policies, so host-target lookup does not degrade in policy-only mode and maintains compatibility with existing channels; canonical order: `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`.
- Additional focus of the current iteration: stabilizing runtime facts parity (`locale`/`oauth_app_id`) and maintaining deterministic contract in tests/docs.

## Execution checkpoint

- Current phase: server_artifact_cleanup
- Last checkpoint: channel admin native server functions now consume `HostRuntimeContext::db_clone()` instead of Loco `AppContext`, the package no longer depends on `loco-rs`, and the REST secondary path remains isolated in `admin/src/transport/rest_adapter.rs`.
- Next step: Collect full Rust runtime contract evidence for `ChannelReadPort` and full server middleware test evidence; until Rust runtime evidence FBA remains `in_progress`, but fallback smoke profiles are now locked by dedicated no-compile executable verifier, resolution-order decision by a fast source verifier, and `rustok-channel-admin` SSR compile evidence is present.
- Open blockers: Full server middleware/runtime contract test evidence is still pending; `cargo check -p rustok-channel-admin --features ssr` passed in this iteration.
- Hand-off notes for next agent: Keep channel admin UI calls behind `transport`, and route-selection policy in `core` or shared route helpers; do not return raw transport calls to `ui/leptos/`.
- Last updated at (UTC): 2026-07-08T07:00:42Z

## FFA/FBA readiness

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - Foundation FBA batch update: `npm run verify:channel:fba` now runs `npm run verify:foundation:fba-runtime-smoke`, so `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` is checked together with `tenant`, `index` and `email` runtime fallback evidence instead of only as a standalone channel gate.
  - Boundary readiness update: `crates/rustok-channel/contracts/channel-fba-registry.json`, `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` are locked by `npm run verify:channel:fba`; FBA status is `boundary_ready`, while full Rust runtime contract evidence remains the next step before `transport_verified`.
  - `crates/rustok-channel/admin/src/lib.rs` is now the composition/re-export layer for module-owned admin surface.
  - Runtime facts parity slice: `apps/server/src/middleware/channel.rs` builds `RequestFacts.locale` from `ResolvedRequestLocale.effective_locale` and `RequestFacts.oauth_app_id` from `AuthContextExtension.client_id`; `ChannelResolutionCacheKey` includes both fields to avoid cross-locale/cross-client policy cache reuse, and source-level middleware tests now cover `LocaleEquals`/`OAuthAppEquals` policy selection from real request extensions.
  - Server artifact cleanup slice: `ChannelBootstrapResponse`, policy-set/rule request DTOs, available module/OAuth app bootstrap DTOs and create/update rule payload helpers live in `crates/rustok-channel/src/dto/mod.rs`; `apps/server/src/controllers/channel.rs` consumes those owner contracts and no longer owns local channel REST DTOs or `ResolutionPredicate`/`ResolutionAction` rule mapping.
  - FBA provider slice: `crates/rustok-channel/src/ports.rs` declares `ChannelReadPort` / `channel.read_projection.v1` for channel/default/host-target read projection consumers with shared `rustok_api::PortContext`/`PortError`, tenant-scope preservation, inactive-channel degraded-mode filtering and `PortCallPolicy::read()` deadline semantics; `crates/rustok-channel/contracts/channel-fba-registry.json` plus `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` lock planned contract cases, and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` locks fallback profiles under `npm run verify:channel:fba` through a dedicated no-compile executable smoke verifier; Rust runtime execution remains the next step before `transport_verified`.
  - Resolution contract slice: built-in host fast-path remains a separate layer after header/query selectors and before typed policies; `npm run verify:channel:resolution-contract` locks source order and docs sync for canonical order `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`.
  - Semantic proof-points slice: `npm run verify:channel:proof-points` source-locks `rustok-pages`, `rustok-blog`, `rustok-commerce` and `rustok-forum` as current channel-aware proof points: public REST/GraphQL gates use resolved host `ChannelContext` and `channel_module_bindings`, page/blog publication visibility remains behind metadata `channelSlugs`, commerce preserves channel snapshot in cart/order/pricing flows without a second sales-channel domain, and forum locks topic/reply/SEO filtering through `forum_topic_channel_access` and request channel slug.
  - FBA provider slice: `crates/rustok-channel/src/ports.rs` declares `ChannelReadPort` / `channel.read_projection.v1` for channel/default/host-target read projection consumers with typed `PortContext`/`PortError`, tenant-scope preservation, inactive-channel degraded-mode filtering and read deadline semantics; `crates/rustok-channel/contracts/channel-fba-registry.json` plus `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` lock planned contract cases, and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` locks fallback profiles under `npm run verify:channel:fba` through a dedicated no-compile executable smoke verifier; Rust runtime execution remains the next step before `transport_verified`.
  - `crates/rustok-channel/admin/src/core.rs` contains Leptos-free selection policy for cleaning up URL-owned channel selection.
  - `ChannelPolicySelectionCleanup` / `channel_policy_selection_cleanup` centralize trim, policy-set lookup and stale rule cleanup; Leptos route effect no longer owns this decision logic.
  - `PolicyRuleFormState` and create/edit builders own default priority, fallback action channel and predicate-to-form mapping; Leptos applies the prepared state only to signals.
  - `reorder_policy_rule_ids` owns first/last boundary checking and rule ID reordering; Leptos move-up/move-down handlers only send the prepared order to the transport facade.
  - `PolicyRuleFormState::{create_payload,update_payload}` and `policy_rule_active_update_payload` own optional-text normalization and transport DTO construction for create/edit/toggle flows.
  - `crates/rustok-channel/admin/src/transport/mod.rs` contains module-owned transport facade and fallback policy, `native_server_adapter.rs` contains server-function endpoints, and `rest_adapter.rs` contains REST secondary path; Leptos adapter no longer imports the pre-FFA module `api`.
  - Channel admin native server functions use `HostRuntimeContext::db_clone()` instead of Loco `AppContext`; `rustok-channel-admin` no longer depends on `loco-rs`, and `scripts/verify/verify-channel-admin-boundary.mjs` now locks the Loco-free native runtime boundary alongside the existing FFA split.
  - `crates/rustok-channel/admin/src/ui/leptos/` is the explicit Leptos render adapter directory: `mod.rs` owns `ChannelAdmin`/shared render glue, and runtime context, policy workbench, policy-set card and channel card are isolated in component files; channel operations call only the module-owned transport facade.
  - `scripts/verify/verify-channel-admin-boundary.mjs` locks the split without full Rust compilation: required `ui/leptos/` structure, absence of `api.rs`/legacy `transport.rs`, absence of raw transport calls in UI, Leptos-free `core`, and separation of `#[server]`/`reqwest` into adapter files.
  - `scripts/verify/verify-channel-admin-boundary.test.mjs` adds fixture-based regression coverage for pass path, legacy `api.rs`, legacy flat `transport.rs`, raw adapter calls from UI, inline policy-selection lookup, Leptos-specific core regression, erroneous `#[server]` endpoints in facade/REST adapter and raw REST calls outside `rest_adapter.rs`.
  - `npm run verify:ffa:ui:migration` now runs the channel admin boundary verifier as part of the common FFA verification pipeline.
- Compile-evidence note (2026-07-08): `cargo check -p rustok-channel-admin --features ssr`, `npm run verify:channel:admin-boundary` and `npm run test:verify:channel:admin-boundary` passed for the Loco-free native admin transport. Remaining parity step: collect full server middleware/runtime contract test evidence before moving channel admin row to `phase_b_ready`.

## Scope of work

- keep `rustok-channel` as a domain-owned resolution module, not a host middleware bucket;
- synchronize channel runtime contract, admin UI and manifest metadata;
- evolve typed resolution policies without returning to ad-hoc host logic.

## Current exploration summary

- resolver precedence is already fixed in `crates/rustok-channel/src/resolution.rs`:
  `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`;
- storage and domain layer for policy already exists (`channel_resolution_policy_sets` +
  `channel_resolution_policy_rules`);
- server transport (`apps/server/src/controllers/channel.rs`) is a thin adapter over owner-owned channel DTOs/helpers, RBAC/tenant checks and cache invalidation;
- admin UI (`crates/rustok-channel/admin/src/ui/leptos/`) already covers basic operator flows and
  rollout rule-level lifecycle;
- middleware request facts (`apps/server/src/middleware/channel.rs`) currently passes
  `oauth_app_id = None` and `locale = None`, which means some typed predicates work
  only in synthetic/test scenarios.

## Required changes

### 1) Domain contract (`rustok-channel`)

- add DTO for update lifecycle policy set/rule (rename/active-toggle/rule update/reorder);
- extend `ChannelService` with methods:
  - `update_resolution_policy_set(...)`,
  - `update_resolution_rule(...)`,
  - `reorder_resolution_rules(...)` (bulk or single move);
- lock partial-update contract for `update_resolution_rule(...)`:
  - `priority/is_active/action_channel_id` optional: absence in payload => field unchanged;
  - `host_equals/host_suffix/oauth_app_id/surface/locale` optional patch fields:
    absence => unchanged, empty string => remove corresponding predicate, non-empty value => replace/set predicate with normal validation/normalization;
- lock invariants:
  - tenant ownership for policy set, rule and action channel,
  - deterministic order after reorder (no hidden tie-break drift),
  - inactive rule does not participate in `list_active_resolution_rules`.

### 2) Host transport (`apps/server`)

- extend REST controller `apps/server/src/controllers/channel.rs` for update/reorder/disable policy flows;
- keep current cache invalidation contract (`invalidate_tenant_channel_cache`) for all new write-paths;
- when adding new request payloads, maintain shared validation semantics
  (host normalization, locale normalization, surface whitelist).

### 3) Runtime facts and middleware integration

- bring `RequestFacts` in `middleware/channel.rs` to real runtime:
  - pass `locale` from resolved request locale,
  - pass `oauth_app_id` from auth context (`client_id`);
- adjust middleware ordering in
  `apps/server/src/services/app_router.rs` if necessary, so channel resolver sees required extension data;
- add targeted middleware tests for policy predicates `LocaleEquals` and `OAuthAppEquals`
  in the real request pipeline, not only at the unit-level resolver.

### 4) Admin package (`rustok-channel/admin`)

- close build-profile-selected native parity for policy operations in `admin/src/transport/`
  (`#[server]` path + REST secondary path, like channel/target/module flows);
- extend `PolicyWorkbench` / `PolicySetCard` (`admin/src/ui/leptos/`) to full operator flow:
  - rule active toggle,
  - rule reorder (up/down or explicit priority move),
  - rule edit without delete/recreate;
- when a separate selection state for policy-set/rule appears, maintain URL-owned contract
  through `rustok-api` route keys (no package-local state contract).

### 5) Proof points in domain modules

- extend channel-aware proof points (`pages` / `blog` / `commerce`) only together
  with explicit tests and local documentation;
- for new channel-aware reads, use already resolved host channel context,
  not creating a second selection channel in module-local logic.

## Integration points

| Layer | Component | Current role | Planned change |
|---|---|---|---|
| Domain | `crates/rustok-channel/src/services/channel_service.rs` | create/activate/delete policy lifecycle | update/reorder/disable lifecycle + invariants |
| Domain | `crates/rustok-channel/src/resolution.rs` | execution pipeline and trace | confirm deterministic policy order after reorder |
| Host REST | `apps/server/src/controllers/channel.rs` | thin channel bootstrap/write API | new policy update/reorder endpoints |
| Host middleware | `apps/server/src/middleware/channel.rs` | request -> `RequestFacts` -> `ChannelContext` | locale/oauth facts parity with runtime extensions |
| Host composition | `apps/server/src/services/app_router.rs` | middleware chaining | adjust middleware ordering if necessary |
| Admin transport | `crates/rustok-channel/admin/src/transport/` | facade + explicit native server-function adapter + REST secondary-path adapter after FFA split | add fast boundary verifier for absence of raw transport/API calls in UI |
| Admin UI | `crates/rustok-channel/admin/src/ui/leptos/` | explicit Leptos render adapter directory after FFA split | keep full operator flow behind core/transport boundaries |
| Shared UI routing | `crates/rustok-api/src/route_selection.rs` | channel query keys (`channel_id/target_id/module_slug/oauth_app_id`) + policy edit keys (`policy_set_id/policy_rule_id`) | maintain URL-owned selection contract and dependency cleanup (`policy_set_id -> policy_rule_id`) |

## Stages

### 1. Contract stability

- [x] lock final resolution model `explicit selectors -> built-in target slice -> typed policies -> explicit default -> unresolved`;
- [x] keep domain-owned resolver inside `rustok-channel`;
- [x] maintain sync between runtime contract, admin UI and server middleware tests.

### 2. Policy lifecycle parity

- [x] bring policy trace to admin bootstrap/runtime diagnostics;
- [x] add basic operator flows for policy-set activation and policy-rule authoring/removal;
- [x] add policy rule update/reorder/disable lifecycle at `ChannelService`, REST transport and admin UI control level;
- [x] add targeted tests for deterministic rule order and inactive-rule exclusion;
- [x] decide whether built-in host slice remains a separate fast-path after full policy rollout.

### 3. Admin operator UX parity

- [x] bring `rustok-channel-admin` to operator flow for policy rules (reorder/disable);
- [x] add full rule edit flow (changing predicates/action without delete+recreate);
- [x] align build-profile-selected native `#[server]` transport for policy operations with existing channel CRUD flows;
- [x] when adding policy edit-selection state, lock URL query contract through shared `AdminQueryKey`.

### 4. Runtime integration rollout

- [x] connect real request locale and OAuth app id in `RequestFacts`;
- [x] lock middleware ordering and source-level runtime facts/policy parity with tests in `apps/server`;
- [x] make decision on built-in host slice (`fast-path` vs policy-only mode): keep built-in host fast-path between explicit selectors and typed policies, lock docs/source guardrail `verify:channel:resolution-contract`.

### 5. Semantic expansion

- [ ] return to richer target/connector taxonomy only when real runtime pressure demands it;
- [x] lock current channel-aware proof points (`rustok-pages`, `rustok-blog`, `rustok-commerce`, `rustok-forum`) with no-compile verifier `npm run verify:channel:proof-points` together with local documentation and test markers.
- [ ] extend new channel-aware proof points in domain modules only together with local documentation and tests.

## Verification

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests for resolution order, explicit selectors, policy predicates and default semantics
- `npm run verify:channel:resolution-contract`
- `npm run verify:channel:proof-points`
- targeted channel service tests for policy lifecycle (`create/update/reorder/disable/delete`)

## Update rules

1. When changing resolution/policy contract, update this file first.
2. When changing public/runtime contract, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata and UI wiring, synchronize `rustok-module.toml`.
4. When changing route-selection contract, synchronize `rustok-api` (`AdminQueryKey`) and UI docs.


## Quality backlog

- [x] Update source-level proof-point coverage for current channel-aware scenarios pages/blog/commerce/forum through `npm run verify:channel:proof-points`.
- [x] Verify completeness and currency of `README.md` and local docs for current proof-point guardrails.
- [x] Lock/update verification gates for current module state: `npm run verify:channel:fba` now checks static matrix and no-compile executable runtime fallback smoke without compilation.
- [ ] Collect full Rust runtime fallback evidence to raise FBA above `in_progress`.
