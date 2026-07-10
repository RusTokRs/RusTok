# Implementation plan for `rustok-pages`

Status: pages-owned storage and visual builder contract are already fixed; the module
is transitioning to FBA-consumer mode for the visual builder capability layer and is kept
in steady-state hardening + rollout polish.

## Execution checkpoint

- Current phase: storefront_native_loco_free
- Last checkpoint: `rustok-module.toml` now declares `controllers::axum_router`, which builds `PagesHttpRuntime` from `HostRuntimeContext` plus its typed `TransactionalEventBus` handle and is merged once by generated host composition. REST page/block handlers use `rustok_web::HttpError`; the domain crate no longer depends on Loco or exposes a Loco route-state adapter. Pages admin and storefront retired legacy `src/api.rs`; admin GraphQL operations live in `admin/src/transport/graphql_adapter.rs`, storefront raw transport is split between `storefront/src/transport/graphql_adapter.rs` and `storefront/src/transport/native_server_adapter.rs`, crate roots no longer wire `mod api`, and `verify-pages-ui-boundary.mjs` rejects reintroducing either legacy API module. Public GraphQL/storefront reads now use `SecurityContext::public_read()` when `AuthContext` is absent, while published/channel visibility filtering stays in the pages read path. Storefront native server functions now consume `HostRuntimeContext` plus a typed `TransactionalEventBus` host handle, no longer import Loco `AppContext`, and no longer depend on `loco-rs` or the outbox `loco-adapter` feature.
- Dependency evidence: storefront no-feature/hydrate profiles contain neither `rustok-core` nor backend `rustok-pages`; runtime dependencies are optional and enabled only by `ssr`.
- Next step: Run a real control-plane Wave 0 dry-run on an internal tenant and replace the synthetic packet with actual before/after snapshots; then replace the Wave 1 readiness draft with a real tenant packet only together with owner sign-off and SLO/smoke evidence. For no-compile Wave 1 hold, use `npm run verify:page-builder:wave1-readiness-draft`; for FFA boundary evidence, use the fast `verify-pages-ui-boundary.mjs`; for FBA rollout policy, use `npm run verify:page-builder:consumer:pages`.
- Open blockers: None.
- Hand-off notes for next agent:
  1. Before any pages changes, first cross-check `docs/research/dioxus-ffa-pilot-connectivity-map.md` and this file; do not open a new slice without a clear goal in the tracker.
  2. For code, follow the current pattern: Leptos UI = thin render/bind, formatting/parsing helpers = `core::*`, dual-path (`native #[server]` + GraphQL selected path) do not change.
  3. If the task is not about pages runtime contract, priority shifts to the next module in the wave; only make bugfix/contract-sync changes to pages.
- Latest maintenance update: Leptos admin package now exposes capability surfaces `preview/tree/properties/publish` for `grapesjs_v1` and keeps legacy `blocks` compatibility visible in the same write-path.
- Latest maintenance update: typed builder error catalog parity (`validation/sanitize/runtime/feature-disabled`) is fixed for admin UI + service/runtime relying on `WritePathIssueKind`, `PagesError::FeatureDisabled`, manifest/registry binding and `verify-page-builder-error-catalog-binding.mjs`.
- Latest maintenance update: create-page draft normalization now lives in `admin/src/core.rs` and reuses `rustok-api::normalize_ui_text` / `parse_ui_csv`, while the Leptos layer remains a thin bind/render adapter.
- Latest maintenance update: admin UI received an explicit FFA split `core` + `transport` + `ui/leptos`; GraphQL operations now live in `admin/src/transport/graphql_adapter.rs`, legacy `admin/src/api.rs` is removed, and render/effect code only calls the facade from `admin/src/transport/`.
- Latest FFA update: storefront UI received matching split `core` + `transport` + `ui/leptos`; crate root re-exports `PagesView`, Leptos adapter only calls `storefront/src/transport/`, native server function lives in `storefront/src/transport/native_server_adapter.rs`, GraphQL selected path lives in `storefront/src/transport/graphql_adapter.rs`, legacy `storefront/src/api.rs` is removed. Fast guardrail `scripts/verify/verify-pages-ui-boundary.mjs` locks admin/storefront boundary without full-workspace compile.
- Latest FBA rollout update: manifest `fba.builder_consumer.rollout_policy` now locks control-plane audit trail, mandatory before/after tenant snapshots, keep/rollback decision, owner sign-off, rollback target <= 10 minutes without redeploy, SLO rollback triggers and pilot smoke `preview -> properties -> publish(dry)`; `verify-page-builder-consumer-readiness.mjs pages` checks these markers without compilation.
- Latest legacy bridge update: `verify-page-builder-pages-legacy-bridge.mjs` added to FBA baseline and locks read/bridge semantics for legacy `blocks`: import/create allowed, visual-builder body writes do not delete blocks, update surface does not get a new block write contract, admin/storefront show compatibility evidence.
- Latest FFA maintenance update: admin capability-card presentation helpers (`publish_state_view`, `channel_count_label`, `legacy_block_snapshot_label`) and storefront list/empty-state helpers (`page_link_href`, `page_status_label`, `selected_page_empty_state`) extracted to `core`, and `verify-pages-ui-boundary.mjs` now locks these no-compile boundary markers.
- Latest FFA maintenance update: admin save/publish busy-state helpers (`is_save_action_busy`, `is_publish_action_disabled`) and storefront load-error composition (`load_error_message`) extracted to `core`; fast `verify-pages-ui-boundary.mjs` and its fixture tests lock new no-compile boundary markers.
- Latest quality backlog update: README/docs schema audit refreshed module-owned storage tables and pages-vs-builder ownership split; RBAC regression tests now lock admin/authenticated bypass for draft and page-channel allowlist semantics; explicit `npm run verify:page-builder:error-catalog` entry documents backend/UI error catalog drift gate without Cargo compilation.
- Latest RBAC Wave 1 readiness update: no-compile guardrail `verify-page-builder-pages-rbac-readiness.mjs` now pins RBAC regression coverage and local/central docs sync inside the FBA baseline without running Cargo.
- Latest contract surface maintenance update: no-compile guardrail `verify-page-builder-pages-contract-surface.mjs` and no-compile fallback gates now lock the presence of public contract tests for CRUD/sanitize, builder round-trip, legacy blocks bridge, degraded fallback profiles, menu lifecycle, locale fallback, RBAC/channel visibility and manifest/provider drift; aggregate FBA baseline runs this guardrail without Cargo compilation.
- Latest FFA maintenance update: storefront selected-page empty-state DTO/helper (`selected_page_empty_state`) extracted to `core`, and `verify-pages-ui-boundary.mjs` and fixture suite lock that the Leptos adapter consumes core-owned empty-state policy without direct fallback state ownership.
- Latest Wave 1 hold update: `verify-page-builder-wave1-readiness-draft.mjs` now locks draft-only invariants for pending tenant, draft change-set namespace, pending metric markers, pending approvals, hold rollback reason and absence of waivers; package script `npm run verify:page-builder:wave1-readiness-draft` added without Cargo compilation.
- Latest observability gate update: `crates/rustok-page-builder/contracts/page-builder-correlation-contract.json` and `verify-page-builder-correlation-evidence.mjs` lock no-compile correlation chain `builder write -> pages publish -> storefront read` for Wave 0/Wave 1 packets and source markers in pages publish/storefront read paths.
- Latest FFA maintenance update: admin table item state/fallback mapping (`admin_page_list_item_view`) and storefront published list item mapping (`storefront_page_list_item_view`) extracted to `core`, and package-level `verify:pages:ui-boundary` scripts restored after JSON drift without Cargo compilation.
- Latest FFA maintenance update: admin table row action busy/label mapping (`admin_page_row_action_state`, `admin_page_row_action_labels`) extracted to `admin/src/core.rs`, Leptos adapter left as thin render/callback layer, and `verify-pages-ui-boundary.mjs` locks new no-compile markers.
- Latest FFA maintenance update: admin write-path issue banner view model (`issue_banner_view`) extracted to `admin/src/core.rs`; Leptos adapter now only receives localized strings and renders core-owned class/label/guidance, and `verify-pages-ui-boundary.mjs` locks no-compile marker.
- Latest FFA maintenance update: admin issue-banner CSS policy is now also locked by no-compile guardrail: Leptos adapter renders `banner.class_name` from `issue_banner_view`, and direct `core::issue_banner_class` call from UI is considered a boundary regression.
- Latest FFA maintenance update: admin properties/compatibility view models (`page_properties_view`, `compatibility_warning_view`) and storefront published-list empty/header view models (`published_pages_empty_state`, `published_pages_header_view`) extracted to `core`; `verify-pages-ui-boundary.mjs` locks these no-compile markers without changing native/GraphQL transport.
- Latest Loco-exit update: storefront native server function transport reads DB and `TransactionalEventBus` from `HostRuntimeContext`; the package manifest no longer enables `loco-rs` or `rustok-outbox/loco-adapter`, and `verify-pages-ui-boundary.mjs` plus the shared API surface guardrail pin that boundary.

- PB-FBA-1 platform sync note: [Page Builder Implementation Plan](../../../docs/modules/page-builder-implementation-plan.md) contains delivery slices and exit criteria for Wave 0 hand-off; pages track should be updated synchronously via dependency notes.
- PB-FBA-1 execution note: sync with central section `8.5 Execution backlog` accepted as active queue (`PB-FBA-1A..1D`, focus Week1=P0/P1, Week2=P2/P3).
- PB-FBA-1A update: `consumer_min_version = "1.0"` added to `fba.builder_consumer`, and machine-readable registry `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` is now checked via `verify-page-builder-contract-registry.mjs` and aggregate baseline gate.
- PB-FBA-1B host update: `pages_builder_fallback_*` gate covers all baseline profiles (`all_on`, `publish_off`, `preview_off`, `builder_off`) on service boundary and admin/storefront host helpers: read/list remain stable, disabled capabilities return typed `FeatureDisabled`, storefront render does not require builder capability.
- PB-FBA-1B catalog update: `fba.builder_consumer.error_catalog`, `error_codes` and `degraded_mode_errors` synchronized with provider metadata, FBA registry and runtime constants; aggregate baseline gate now includes anti-drift error-catalog binding check.
- PB-FBA-1B Next parity update: `apps/next-admin` save-flow displays the same typed catalog (`validation/sanitize/runtime/feature-disabled`) and operator guidance for `FEATURE_DISABLED`; baseline gate includes static parity-check for Next Admin.
- PB-FBA-1B Leptos parity update: module-owned Leptos admin shows localized operator guidance for `validation/sanitize/runtime/feature-disabled`; baseline gate includes static parity-check for `rustok-pages-admin`.
- PB-FBA-1B Flutter parity update: `rustok_mobile/packages/app_core` contains shared mapper for the same typed catalog and `FEATURE_DISABLED` guidance; baseline gate includes static parity-check for Flutter app-core.
- PB-FBA-1B Flutter hand-off contract update: `crates/rustok-page-builder/contracts/page-builder-flutter-wave-handoff.json` and `verify-page-builder-flutter-handoff.mjs` lock that Wave hand-off requires device/runtime evidence via shared mobile mapper, but does not carry FBA thresholds or toggle semantics into mobile registry; actual packet remains a Wave 1 blocker.
- PB-FBA-1D synthetic observability update: Wave 0 dry-run packet now contains baseline metrics, pilot SLO thresholds/evaluation and 2 correlation trace samples (`builder_write -> pages_publish -> storefront_read`); `verify-page-builder-wave-evidence-packet.mjs` blocks threshold drift, placeholder traces, missing spans and incomplete correlation path. Additional `verify-page-builder-correlation-evidence.mjs` locks this builder write -> pages publish -> storefront read contract at the evidence packets, docs and source markers level. Actual tenant metrics/traces remain Wave hand-off evidence.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress` (consumer baseline for `rustok-page-builder`; remote runtime profile not yet enabled)
- Structural shape: `core_transport_ui`
- Evidence:
  - public read authority slice: pages GraphQL request security resolves authenticated snapshots through `SecurityContext::from_permission_snapshot(...)` and resolves missing-auth public reads to `SecurityContext::public_read()`; storefront native slug reads use the same anonymous authority and preserve published/channel visibility filtering;
  - module plan synchronized with central FFA/FBA readiness board;
  - FBA consumer metadata synchronized with `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`, `rustok-module.toml` and baseline gate;
  - further status promotion is done only together with verification evidence and local+central docs update;
  - FFA maintenance slice: create-page draft normalization, channel slug CSV parsing and route text checks reuse shared UI helpers from `rustok-api` without changing native/GraphQL transport;
  - FFA admin slice: Leptos render/effect adapter lives in `admin/src/ui/leptos.rs`, transport facade in `admin/src/transport/`, GraphQL adapter in `admin/src/transport/graphql_adapter.rs`; external GraphQL contract unchanged, legacy `admin/src/api.rs` removed and locked as forbidden by `verify-pages-ui-boundary.mjs`;
  - FFA storefront slice: Leptos render/bind adapter lives in `storefront/src/ui/leptos.rs`, crate root only wires modules/re-exports `PagesView`, transport facade in `storefront/src/transport/mod.rs`, GraphQL adapter in `storefront/src/transport/graphql_adapter.rs`, native server adapter in `storefront/src/transport/native_server_adapter.rs`; legacy `storefront/src/api.rs` removed and fast boundary guardrail `scripts/verify/verify-pages-ui-boundary.mjs` locks admin/storefront split, Leptos-free core, docs sync, Loco-free native transport, and typed host handles for DB/event bus.
- Last verified at (UTC): 2026-07-08T00:00:00Z
- Owner: `rustok-pages` module team

## PB-FBA immediate sprint (continuing page builder development)

### Sprint goal

Transition `rustok-pages` from "handshake in progress" status to a verifiable FBA-consumer baseline that can be scaled to subsequent modules using the same pattern.

### Sprint scope (must-have)

- [x] Typed fallback matrix: `builder_off`, `preview_off`, `publish_off` with expected runtime/error outcomes.
- [x] Unified builder error catalog for `validation/sanitize/runtime/feature-disabled` without divergence between GraphQL, `#[server]` and UI adapters.
- [x] CI fallback gate for profiles `all_on`, `publish_off`, `preview_off`, `builder_off`: provider runtime gate and `rustok-pages` service/admin/storefront consumer fallback gate connected to baseline check.
- [x] Contract freeze anti-drift: `builder_contract_version`, `consumer_min_version`, capability set and fallback profile names fixed in machine-readable registry and checked by aggregate baseline gate.

### Fallback matrix (admin/list/read/publish snapshots)

This matrix is the consumer-side snapshot for `rustok-pages` and must match the provider matrix in `rustok-page-builder::rollout`. Read/list/menu paths remain owned by pages and must not depend on builder capability endpoint availability.

| Profile | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Disabled capabilities |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview`, `publish` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `preview`, `tree`, `properties`, `publish` |

Operational notes:

1. `builder_off` does not disable pages-owned list/read/menu runtime; admin visual path must show read-only fallback instead of 5xx.
2. `publish_off` returns typed `feature-disabled`/`typed_feature_disabled_error` only on the builder publish path; legacy/direct read paths remain stable.
3. `preview_off` hides or blocks preview capability but must not prohibit properties/tree reading if `builder.properties.enabled=true`.

- [x] Wave 0 evidence template: flags snapshot + smoke output + observability snapshot + keep/rollback decision (`crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json`).
- [x] Synthetic Wave 0 dry-run packet for all baseline profiles: `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json` (validates shape, fallback semantics, baseline metrics, SLO thresholds/evaluation and at least 2 correlation trace samples; does not replace actual tenant evidence).

### Out of scope (for this sprint)

- Extension of visual editor functionality beyond capability contract.
- Any vendor-specific surface outside `grapesjs_v1`.
- Changing ownership boundaries (pages runtime owner vs external builder capability provider).


## Scope of work

- maintain `rustok-pages` as owner of page, block and menu runtime contract;
- synchronize visual builder semantics as external FBA capability layer, visibility rules and local docs;
- prevent page read/write paths from reverting to shared storage.

## Current state

- pages, page bodies, blocks and menus already work on module-owned persistence;
- GraphQL/REST adapters and Leptos admin/storefront packages already live inside the module;
- `grapesjs_v1` is fixed as the canonical visual page-builder write-path;
- visibility contract already uses typed relation `page_channel_visibility`;
- write-path UX for page builder now uses a unified error pattern `validation/sanitize/runtime` and contract-safe JSON handling for `body.contentJson`.

## FBA migration frame (`pages` as consumer reference builder module)

- `rustok-pages` continues to own page/menu lifecycle and publish pipeline.
- Builder-domain (`preview/tree/properties/publish`) is treated as external capability-provider.
- Module docs and runtime metadata lock the prohibition on reverting to pages-local ownership of the visual builder runtime.
- Legacy block-driven path is maintained as a compatibility-bridge with explicit sunset roadmap.


## Dedicated page-builder track (FBA hand-off scope)

### Scope now

- pages runtime remains owner for `page/menu/visibility/routing`.
- visual builder write-path works through external capability-provider (`preview/tree/properties/publish`).
- module-level runbook must describe degraded mode when builder capability is disabled.

### Acceptance criteria for hand-off

- [x] Admin UI shows clear fallback-state when `builder.enabled=false`.
- [x] Storefront read-path does not depend on builder capability endpoint availability.
- [x] Publish endpoint correctly returns typed runtime error when `builder.publish.enabled=false`.
- [x] Legacy blocks path works in read/bridge mode without write surface expansion (`verify-page-builder-pages-legacy-bridge.mjs`).

Legacy blocks path works in read/bridge mode; visual-builder body writes must not delete legacy blocks or create a block write surface.
- [x] Toggling tenant flags does not require redeploy and leaves list/read surfaces accessible.

### Tenant switch procedure (operational checklist)

Manifest source of truth: `fba.builder_consumer.rollout_policy` in `rustok-module.toml`.

1. Capture `before` snapshot of flags and module health in `control_plane_builder_wave_audit`.
2. Apply change-set (`builder.enabled`, `builder.preview`, `builder.properties`, `builder.publish`).
3. Run targeted smoke (`list -> open -> preview -> save-draft -> publish-dry`) and mandatory pilot smoke `preview -> properties -> publish(dry)`.
4. Validate logs/metrics (`sanitize`, `runtime`, `publish_latency`).
5. Capture `after` snapshot + decision note (`keep/rollback`) + owner sign-off in the same audit trail.

Rollback trigger:

- runtime errors above alert threshold;
- publish latency p95 above target SLO for 10 minutes;
- sanitize failures above alert threshold;
- storefront read regression on published pages.

Rollback target: toggling tenant flags back must take <= 10 minutes and does not require redeploy of `pages` runtime; pages-owned list/read/menu surfaces remain available in all degraded builder profiles.

## Stages

### 1. Contract stability

- [x] close storage split for pages, blocks and menus;
- [x] lock builder contract `markdown | rt_json_v1 | grapesjs_v1`;
- [x] maintain compatibility surface for legacy block-driven pages;
- [x] maintain sync between runtime contracts, UI packages and module metadata;
- [x] contract tests cover all public use-cases for already shipped pages runtime surfaces (`verify-page-builder-pages-contract-surface.mjs` locks coverage map without compilation).
- [x] lock in runtime metadata that the builder capability layer is an external provider boundary.

### 2. Product hardening

- [ ] maintain GraphQL and REST surfaces synchronized when page builder flows change;
- [ ] evolve page/menu observability and write-path metrics under real operational pressure;
- [ ] document policy for authenticated/admin bypass and stricter visibility invariants, if it changes.
- [x] describe tenant-level toggle policy for capability surfaces (`builder.preview/tree/properties/publish`) without degrading core pages runtime.

### 3. Operability

- [ ] cover page/block/menu lifecycle with targeted integration tests;
- [ ] document new runtime guarantees concurrently with visual builder and visibility contract changes;
- [x] synchronize local docs, README and central references when module boundary changes.
- [x] add FBA runbook: partial disable capability layer + fallback behavior for admin/storefront paths.

## FBA execution backlog (`pages` as consumer reference builder module)

### B1. Contract & metadata hardening

- [x] Update runtime metadata/manifest: explicitly specify external `builder capability-provider` and supported capability surfaces (`preview/tree/properties/publish`) — see `rustok-module.toml` (`dependencies.page_builder`, `fba.builder_consumer`).
- [x] Add contract-version marker for anti-drift checks between `pages`, Next/Leptos adapters and reference builder (`contract_version = "1.0"` in metadata consumer/provider link).
- [x] Add `consumer_min_version = "1.0"` and synchronize machine-readable registry `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` with manifest provider/consumer contract values.
- [x] Lock machine-readable degraded modes (`builder_disabled`, `publish_disabled`, `preview_disabled`) in `fba.builder_consumer.degraded_modes`.

### B2. Fallback & error semantics

- [x] Establish unified typed error catalog for builder-related runtime errors (`validation/sanitize/runtime/feature-disabled`) and link it to `degraded_modes` via machine-readable manifest/registry gate.
- [x] Add fallback snapshots to docs for admin/list/read/publish surfaces.
- [x] Ensure that baseline profiles `all_on`, `publish_off`, `preview_off`, `builder_off` do not break page read/list/menu paths on service fallback gate and host-level admin/storefront helper checks; Next Admin, Leptos and Flutter app-core typed-error parity locked; runtime device-level evidence remains in Wave hand-off.

### B3. Operability & rollout

- [x] Tie tenant switch checklist to control-plane audit trail (before/after snapshots + decision) via `fba.builder_consumer.rollout_policy.audit_trail`.
- [x] Synchronize rollback triggers with platform SLO policy (p95 publish, runtime error-rate, sanitize failures) in manifest rollout policy.
- [x] Add runbook-note for pilot-tenants: mandatory smoke `preview -> properties -> publish(dry)`.

### B4. Verification gates

- [x] Include fallback regression checks in `cargo xtask module test pages` (or equivalent CI gate): `verify-page-builder-fba-baseline.mjs` runs provider runtime gate, registry anti-drift gate, error-catalog binding gate, Next Admin parity gate, Leptos admin parity gate, Flutter parity gate, Wave evidence-template gate, synthetic evidence packet gate, `rustok-pages` service/admin/storefront fallback gates across all four baseline profiles and no-compile legacy blocks read/bridge guardrail `verify-page-builder-pages-legacy-bridge.mjs`.
- [x] Add targeted integration checks for `all_on`, `publish_off`, `preview_off`, `builder_off` at `pages` service/transport boundary level (`pages_builder_fallback_*` checks).
- [x] Lock evidence-template for Wave hand-off (platform + pages owner approval): `crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json` + `verify-page-builder-wave-evidence-template.mjs`.

## Wave 0 execution checklist (operational minimum for `pages`)

### C1. Toggle profiles (mandatory)

- [x] `all_on`: `builder.enabled=true`, `preview/properties/publish=true` (service + admin/storefront host fallback gate).
- [x] `publish_off`: `builder.publish.enabled=false`, publish-path returns typed `feature-disabled` error, read/list paths stable.
- [x] `preview_off`: preview capability unavailable, read/list surfaces do not degrade (service + admin/storefront host fallback gate).
- [x] `builder_off`: service read/list paths stable, publish-path returns typed `feature-disabled` error; UI read-only fallback remains Wave evidence.

### C2. Evidence package for each profile

- [~] before/after flag and module health snapshots: synthetic dry-run packet locked; actual tenant snapshots still pending.
- [~] smoke output: `list -> open -> preview -> save-draft -> publish-dry` (synthetic expected outcomes locked; actual control-plane smoke output still pending).
- [~] observability snapshot: `sanitize`, `runtime`, `publish_latency` (synthetic placeholders locked; actual metrics still pending).
- [~] `keep/rollback` decision + owner signature in control-plane audit trail (synthetic `keep` decision locked; actual owner sign-off still pending).

### C3. Exit criteria for Wave 1

- [x] service-level fallback regression checks and admin/storefront host-helper static checks green on current commit; Next/Flutter typed error parity still required for Wave 1.
- [x] no RBAC regression for editor/moderator/admin in builder-related scenarios: `crates/rustok-pages/tests/rbac.rs` locks manager publish prohibition, customer draft restrictions, admin draft bypass and page-channel allowlist bypass; fast no-compile gate `verify-page-builder-pages-rbac-readiness.mjs` included in FBA baseline; `verify-page-builder-pages-contract-surface.mjs` locks public contract-surface coverage without Cargo compilation.

No RBAC regression for editor, moderator, or admin builder scenarios is required before Wave 1 promotion.
- [~] confirmed rollback execution <= 10 minutes without `pages` runtime redeploy: manifest target locked, actual tenant evidence expected in real Wave 0 dry-run.
- [x] Wave 1 readiness draft remains a safe hold artifact: no-compile guardrail checks pending tenant/sign-off/metrics markers and prohibits waivers until actual tenant evidence appears.

## Verification

Contract tests cover all public use cases for pages, including CRUD, sanitization, builder fallback, legacy blocks, menus, locale fallback, RBAC, and channel visibility.

- `cargo xtask module validate pages`
- `cargo xtask module test pages`
- targeted tests for CRUD, body sanitize, legacy blocks bridge, menus, locale fallback, builder round-trip, degraded fallback profiles, RBAC and channel visibility; no-compile coverage guardrail: `npm run verify:page-builder:pages:contract-surface`

## Update rules

1. When changing pages runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing dependency graph, UI wiring or visibility semantics, synchronize `rustok-module.toml`.
4. When changing shared rich-text expectations, also update related docs in `rustok-content`.
5. When changing page-builder contract, synchronously update dependency-notes in `docs/modules/page-builder-implementation-plan.md` and `docs/research/flutter.md`.


## Quality backlog

### Tests
- [x] Add integration tests for degraded modes (publish_off, preview_off, builder_off) at rustok-pages service boundary level (`crates/rustok-pages/tests/page_service_kind_guard.rs` - `pages_builder_fallback_*` tests).
- [x] Write automated tests to verify correct FeatureDisabled error mapping display in Leptos and Next.js UI components (`verify-page-builder-leptos-admin-parity.mjs` / `verify-page-builder-next-admin-parity.mjs`).
- [x] Implement tests for bypassing visibility restrictions (channel visibility, page channel visibility) for roles with administrative rights (`crates/rustok-pages/tests/rbac.rs`).
- [x] Lock public contract tests map with no-compile guardrail `verify-page-builder-pages-contract-surface.mjs` and include it in aggregate FBA baseline.

### Documentation
- [x] Conduct README.md and docs/README.md audit against actual DB schema (tables `pages`, `page_translations`, `page_bodies`, `page_blocks`, `page_channel_visibility`, `menus`, `menu_translations`, `menu_items`, `menu_item_translations`).
- [x] Document degraded mode behavior and toggle policy for tenant flags without requiring redeploy (`docs/README.md` + rollout policy section).
- [x] Lock architectural responsibility split between pages runtime boundary and external page-builder provider in README.

### Verification Gates
- [x] Integrate verify-page-builder-pages-legacy-bridge.mjs script into pre-commit hooks.
- [x] Implement verify-pages-ui-boundary.mjs execution in pre-push pipeline.
- [x] Implement automatic error schema cross-check in CI between backend layer and UI components to prevent error type drift (`npm run verify:page-builder:error-catalog`, also part of FBA baseline).


## B3 operability rollout guardrail (2026-06-13)

- Manifest rollout policy locks `control_plane_builder_wave_audit` as mandatory audit trail for before/after snapshots, keep/rollback decision and owner sign-off.
- Pilot tenants must perform smoke `preview -> properties -> publish(dry)` in addition to the general `list -> open -> preview -> save-draft -> publish-dry`.
- Rollback triggers synchronized with platform SLO policy: runtime error-rate, publish p95 over 10 minutes, sanitize failures and storefront published-read regression.
- Rollback target locked as <= 10 minutes without `pages` runtime redeploy; core pages-owned list/read/menu paths remain stable when builder capabilities are disabled.
- Verification: `npm run verify:page-builder:consumer:pages` checks rollout policy markers without Cargo compilation.
