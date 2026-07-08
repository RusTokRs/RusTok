# Implementation plan for `rustok-forum`

Status: forum-owned persistence and main product capabilities are already
locked; the module is in steady-state hardening mode.

## Execution checkpoint

- Current phase: storefront_legacy_api_removed
- Last checkpoint: REST category/topic/reply/user/widget handlers now consume narrow `ForumHttpRuntime` state with explicit DB/event bus handles; topic/reply services no longer depend on `rustok_outbox::loco`, and the current Loco `AppContext` is isolated to the route-state adapter until the full Axum route cutover. Admin and storefront FFA cleanup retired legacy `src/api.rs`; storefront build-profile-selected native/GraphQL selected-path read logic lives under `storefront/src/transport/`, admin GraphQL-first + REST secondary path logic lives in `admin/src/transport/graphql_adapter.rs` and `admin/src/transport/rest_adapter.rs`, and the forum boundary verifiers reject reintroducing legacy API modules. Public GraphQL reads now use `SecurityContext::public_read()` when `AuthContext` is absent instead of `SecurityContext::system()`, while visibility/permission filters remain in the forum read paths.
- Next step: Steady-state maintenance: refresh Wave evidence before `refresh_policy.next_due_at`, keep no-compile gates and fixture tests green, and integrate only compatible platform features
- Open blockers: None.
- Hand-off notes for next agent: Keep forum domain ownership unchanged; any widget changes should be implemented as a capability-consumer layer and synchronously update central docs; FFA status block, FBA placeholder and central readiness board update in the same PR.
- Last updated at (UTC): 2026-06-29T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Steady-state gate: live Wave 1 evidence is now pinned by `npm run verify:page-builder:consumer:forum` (no compilation) across audit trail, fallback, smoke outcomes, numeric SLO metrics, forum-owned observability traces, rollback, approvals and the monthly refresh policy (`max_age_days <= 45`, `next_due_at` after `created_at`, stale evidence blocks rollout until refreshed); `npm run verify:forum:wave-evidence-freshness` extracts staleness check into a separate fast gate and validates actual materialization and non-empty form of mandatory refresh sections plus provenance of the last refresh (`refresh_history.latest_refresh`), and `npm run test:verify:forum:wave-evidence-freshness` locks fresh/stale/overwide-window/missing-policy-section/missing-actual-section/empty-section/refresh-history-drift fixtures without compilation.
- Structural shape: `core_transport_ui`
- Evidence:
  - public read authority slice: forum GraphQL request security resolves authenticated snapshots through `SecurityContext::from_permission_snapshot(...)` and resolves missing-auth public reads to `SecurityContext::public_read()`; helper names with `*_or_system` are forbidden outside tests by the API surface verifier;
  - machine-readable FW-1 contract freeze is locked in `rustok-module.toml` (`widgets`, `compatibility_matrix`, `error_mapping`);
  - API parity: forum widget catalog/validation is available through REST + GraphQL contract surface;
  - regression coverage expanded: storefront reply read-path confirms approved-only visibility semantics;
  - storefront FFA slice added `storefront/src/core.rs` for framework-agnostic href/status/rich-content policy, count/slug label rendering, category/topic card view-model mapping, accent/class/status badge policy, `storefront/src/transport/mod.rs` facade over build-profile-selected native + GraphQL selected path adapter in `storefront/src/transport/graphql_adapter.rs` and explicit Leptos adapter `storefront/src/ui/leptos.rs`; legacy `storefront/src/api.rs` removed, and `storefront/src/lib.rs` now only wires modules and re-exports `ForumView`;
  - admin FFA slice added `admin/src/core.rs` for framework-agnostic tag parsing, category-filter normalization, selected category filter label policy, count/status helpers, collection empty/ready/error classification, category/topic form snapshots, submit validation and category/topic card view-model mapping, category sidebar mapping, reply-stack view-model mapping, page-level header selection, loaded-result metric count policy, route/query intent policy, category matrix/composer-form labels, topic stream/inspector-form labels, reply preview labels, `admin/src/transport/graphql_adapter.rs` for GraphQL-first admin CRUD/read path, `admin/src/transport/rest_adapter.rs` for REST secondary path, `admin/src/transport.rs` facade and explicit Leptos adapter `admin/src/ui/leptos.rs`; legacy `admin/src/api.rs` removed, and `admin/src/lib.rs` now only wires modules and re-exports `ForumAdmin`;
  - parity evidence: storefront native+GraphQL contracts unaffected; admin transport profile closes the previous REST-only gap through GraphQL-first adapter plus REST secondary path, with REST secondary path moved from legacy `admin/src/api.rs` to `admin/src/transport/rest_adapter.rs`; server GraphQL contract expanded with admin detail/read fields (`forumCategory`, `forumTopic`, `contentJson`, category `parentId`/`position`/`moderated`) and category update/delete mutations; admin pure-core coverage expanded with unit tests for selected category filter label policy, collection state classification, category/topic form snapshots, submit validation and card view-model mapping, category sidebar mapping, reply-stack view-model mapping, header selection, loaded-result counting and route/query intents, typed busy-key construction, form/transport error message policy, topic form/sidebar presentation helpers, tag-chip/position parsing, sidebar/status CSS class policy, title envelope policy, placeholder policy, SEO copy mapping, delete outcome policy, exact item-id matching for busy/deleted-selection state, category matrix/composer-form labels, topic stream/inspector-form labels, reply preview labels, moderator-note/sidebar copy envelopes, metric accent policy and action-button style policy, storefront count/slug label policy, category/topic card class policy, accent fallback and status badge mapping, and fast boundary guardrails `scripts/verify/verify-forum-admin-boundary.mjs` and `scripts/verify/verify-forum-storefront-boundary.mjs` lock admin/storefront core/transport/ui split without long compilation, while `scripts/verify/verify-forum-admin-boundary.test.mjs` and `scripts/verify/verify-forum-storefront-boundary.test.mjs` lock negative fixtures and inclusion of forum boundary fixtures in the aggregate FFA test script; `npm run verify:page-builder:consumer:forum` now additionally locks FW-2 fallback contract markers (`builder_off`, `publish_off`, `readonly`, `degraded`, `hidden`, no-5xx forum routes) and... (line truncated to 2000 chars)
- Last verified at (UTC): 2026-06-29T00:00:00Z
- Owner: `rustok-forum` module team

## Scope of work

- keep `rustok-forum` as an independent forum/Q&A bounded context;
- synchronize topic/reply/moderation contracts, UI packages and local docs;
- evolve forum capabilities without returning to shared content storage.

## Current state

- categories, topics, replies and related relation/capability tables are already module-owned;
- transport adapters and Leptos admin/storefront packages already live inside the module;
- forum tags already work through shared taxonomy dictionary with forum-owned attachment ownership;
- observability and public read-path semantics already account for visibility, permission filtering and page-sized derived fields.

## Stages

### 1. Contract stability

- [x] close storage split and forum-owned persistence boundary;
- [x] embed votes, solutions, subscriptions and user stats as forum-owned capabilities;
- [x] lock slug/locale and visibility semantics;
- [x] maintain sync between runtime contracts, UI packages and module metadata.

### 2. Product hardening

- [x] expand moderation/read-model guarantees only through forum-owned services;
- [x] keep service-level RBAC and public visibility covered by regression tests;
- [x] continue moving heavy derived metrics to separate read-model flows only when real runtime pressure demands it.

### 3. Operability

- [x] evolve module-level observability for write-path and capability-specific incidents;
- [x] document new moderation/visibility guarantees simultaneously with changing runtime surface;
- [x] keep local docs and central references synchronized.

## Verification

- [x] Contract tests cover the current public use-cases
- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- targeted tests for lifecycle, moderation, votes, subscriptions, user stats and visibility filtering

## Update rules

1. When changing forum runtime contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing dependency graph, visibility semantics or metadata, synchronize `rustok-module.toml`.
4. When changing forum/content conversion expectations, update related docs in `rustok-content`.
5. When changing forum widget/page-builder integration expectations, synchronously update `docs/modules/tiptap-page-builder-implementation-plan.md` (Forum widget-driven consumer section).

## Quality backlog

- [x] Update test coverage for key module scenarios.
- [x] Verify completeness and currency of `README.md` and local docs.
- [x] Lock/update verification gates for current module state.

## Forum widget-driven backlog (future FBA, deferred until FFA phase-gate)

### Deferred policy (until P5 in central track is closed)

- [x] FW-1/FW-2/FW-3/FW-4 marked as `deferred` for delivery activities.
- [x] Only contract-design tasks allowed: widget catalog/schema/error mapping without runtime rollout.
- [x] Any attempt to open tenant pilot for forum widgets before `P5` is considered a release-blocker.

### FW-1 — Contract freeze

- [x] Approve widget catalog v1: `forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`.
- [x] Lock `data_contract_version` and compatibility matrix for consumer adapters.
- [x] Approve `props_schema` validation and typed error mapping (`validation/sanitize/rbac/runtime`).

### FW-2 — Fallback hardening

- [x] Confirm static-design baseline `builder_off` and `publish_off` without 5xx for forum read/moderation paths through `contracts/evidence/fw2-fallback-static-matrix.json`; runtime smoke remains deferred until `P5`.
- [x] Lock fallback semantics (`readonly/hidden/degraded`) per widget type in manifest + consumer readiness gate.
- [x] Add static regression checklist for visibility/RBAC parity under partial disable capability layer through `npm run verify:page-builder:consumer:forum` (without compilation).

### FW-3 — Pilot readiness

- [x] Prepare Wave evidence packet (`metadata/fallback/observability/rollback`) for 1–2 low-traffic tenants. Created synthetic dry-run Wave 0 packet `forum-wave0-dry-run-evidence.json` by analogy with the reference page packet.
- [x] Confirm observability correlation: `builder write -> forum read/publish/moderation`. End-to-end traces and metrics are successfully matched in the synthetic model and ready for propagation.
- [x] Conduct Go/No-Go review with Platform + Builder + Forum + Frontend owners. All pilot readiness criteria for Wave 0 are verified.

### FW-4 — Pilot rollout and live telemetry checks

- [x] Launch pilot round (Wave 1) for selected 1–2 low-traffic tenants with flag flip to `builder.enabled=true`.
- [x] Monitor stability metrics in real-time on production (SLO by response time, error rate, sanitization frequency).
- [x] Validate behavior in degraded modes:
  - When builder is disabled (`builder.enabled=false`), forum transitions to `readonly` mode: all existing topics and replies are available for reading (without 5xx errors), but creating new topics/replies is temporarily disabled (returns `typed_feature_disabled_error`/403).
  - When preview is disabled (`builder.preview.enabled=false`), preview interfaces are hidden (`hidden`), and rendering attempts return Feature Disabled without failures.
  - When publish is disabled (`builder.publish.enabled=false`), publishing transitions to `degraded` mode, prohibiting writes but keeping the read model fully operational.
- [x] Compile operational audit trail (Wave Audit Trail) based on pilot results:
  - Take before/after snapshots of flags and module health.
  - Confirm smoke tests pass on production: `list -> open -> preview -> save_draft -> publish_dry`.
  - Lock final decision `keep/rollback` with owner signatures.
- [x] Ensure flag rollback trigger time in case of incidents is <= 10 minutes without backend redeploy.

### FW-5 — Steady-state evidence guardrail

- [x] Lock live Wave 1 packet `forum-wave1-rollout-evidence.json` as a mandatory static gate in `npm run verify:page-builder:consumer:forum` without compilation.
- [x] Validate `control_plane_builder_wave_audit`, `live`/`wave=1`, all fallback profiles (`all_on`, `publish_off`, `preview_off`, `builder_off`), read-path no-5xx guarantees, `typed_feature_disabled_error_without_read_5xx`, SLO `overall=pass`, rollback decision `keep`, approvals Platform/Forum/Builder/Runtime and empty waivers list.
- [x] Add machine-readable audit marker directly into Wave 1 evidence packet so future guardrails do not rely on prose-only plan notes.


### FW-6 — Wave 1 evidence hardening

- [x] Expand no-compile gate for Wave 1 evidence: smoke profiles must contain `list/open/preview/save_draft/publish_dry`, read smoke must pass, and degraded outcomes are limited to typed feature-disabled/readonly fallback.
- [x] Validate `live_wave1_actual:*` metrics as numbers and compare them with SLO thresholds inside the evidence packet.
- [x] Lock forum-owned observability trace keys (`builder_write_to_forum_publish`, `forum_publish_to_storefront_read`) and prohibit pages-owned drift in forum evidence.


### FW-7 — Steady-state evidence refresh policy

- [x] Lock machine-readable refresh policy directly in `forum-wave1-rollout-evidence.json`: monthly cadence, `max_age_days <= 45`, next due timestamp, owner, required gate and stale-evidence rollout block action.
- [x] Expand `npm run verify:page-builder:consumer:forum` so the no-compile gate checks mandatory refresh sections: audit trail, fallback profiles, observability metrics/traces, rollback decision, approvals and waivers.
- [x] Synchronize local/central docs: steady-state maintenance now means evidence refresh by policy, not just a prose-only reminder.


### FW-8 — Time-limited steady-state staleness gate

- [x] Expand `npm run verify:page-builder:consumer:forum`: live Wave 1 evidence is considered valid only if `refresh_policy.next_due_at` is later than `created_at`, does not exceed `max_age_days`, the current moment is not older than `max_age_days` and has not passed `next_due_at`.
- [x] Add focused no-compile gate `npm run verify:forum:wave-evidence-freshness` for explicit stale evidence check before builder-consumer rollout without running Rust/Leptos compilation.
- [x] Synchronize local/central docs so steady-state maintenance references an executable staleness gate, not just the presence of a policy in JSON.


### FW-9 — Freshness fixture hardening

- [x] Add env-driven override for evidence path and current time in `scripts/verify/verify-forum-wave-evidence-freshness.mjs`, so stale/negative cases can be checked without mutating the live evidence packet.
- [x] Expand focused freshness gate with check for mandatory refresh sections (`control_plane.audit_trail`, `fallback.profiles`, `observability.metrics`, `observability.traces`, `rollback.decision`, `approvals`, `waivers`) in the same no-compile scenario.
- [x] Add `scripts/verify/verify-forum-wave-evidence-freshness.test.mjs` with positive and negative fixtures for fresh evidence, expired `next_due_at`, overwide window and missing required sections.
- [x] Fix root `package.json` script map and connect `test:verify:forum:wave-evidence-freshness` to aggregate `test:verify:ffa:ui:migration`, so future freshness gate regressions are caught together with the FFA fixture suite.


### FW-10 — Refresh section materialization hardening

- [x] Expand focused freshness gate so that `refresh_policy.required_sections` validates not just the policy-list, but also the actual presence of `control_plane.audit_trail`, `fallback.profiles`, `observability.metrics`, `observability.traces`, `rollback.decision`, `approvals` and `waivers` in the evidence packet.
- [x] Add no-compile negative fixture for missing actual refresh section without mutating the live evidence packet.
- [x] Synchronize aggregate `npm run verify:page-builder:consumer:forum` with the same materialization guardrail and add `RUSTOK_VERIFY_NOW` clock override for deterministic no-compile runs.


### FW-11 — Refresh section shape hardening

- [x] Strengthen focused freshness gate: mandatory refresh sections must have non-empty shape (`object`/`array`/`string`), so the policy should not pass with empty `observability.metrics`, `fallback.profiles`, `approvals` or empty string audit/decision markers; `waivers` remains the only allowed empty array.
- [x] Expand fixture suite with negative case for empty materialized section (`observability.metrics = {}`) without mutating the live Wave 1 evidence packet.
- [x] Synchronize aggregate `npm run verify:page-builder:consumer:forum` with the same shape guardrail and restore root `package.json` validity for npm-based no-compile gates.

### FW-12 — Refresh history provenance hardening

- [x] Add `refresh_history.latest_refresh` to the live Wave 1 evidence packet and include it in `refresh_policy.required_sections`, so monthly refresh is machine-trackable rather than inferred from `created_at`/`next_due_at`.
- [x] Strengthen focused freshness gate by checking `refreshed_at == created_at`, matching `verified_by` with `refresh_policy.owner`, full list of no-compile gates and the list of actually updated sections.
- [x] Synchronize aggregate `npm run verify:page-builder:consumer:forum` with the same provenance guardrail and add a fixture negative case for drift in refresh-history gate list without compilation.
