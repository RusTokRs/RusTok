# Implementation plan for `rustok-forum`

Status: forum-owned persistence and main product capabilities are already
locked; the module is in steady-state hardening mode.

## Execution checkpoint

- Current phase: storefront_legacy_api_removed
- Last checkpoint: `rustok-module.toml` declares `controllers::axum_router`, which builds `ForumHttpRuntime` from `HostRuntimeContext` plus a typed `TransactionalEventBus` and is merged once by generated host composition. REST handlers use `rustok_web::HttpError`; the domain crate no longer depends on Loco or exposes a route-state adapter. Topic/reply services do not depend on `rustok_outbox::loco`. Admin and storefront FFA cleanup retired legacy `src/api.rs`; storefront build-profile-selected native/GraphQL selected-path read logic lives under `storefront/src/transport/`, admin GraphQL-first + REST secondary path logic lives in `admin/src/transport/graphql_adapter.rs` and `admin/src/transport/rest_adapter.rs`, and the forum boundary verifiers reject reintroducing legacy API modules. Public GraphQL reads now use `SecurityContext::public_read()` when `AuthContext` is absent instead of `SecurityContext::system()`, while visibility/permission filters remain in the forum read paths.
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

## Steady-state work

- [ ] Maintain forum domain guarantees and refresh module documentation whenever a runtime contract changes.

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
5. When changing forum widget/page-builder integration expectations, synchronously update `docs/modules/page-builder-implementation-plan.md`.

## Quality backlog

- [x] Update test coverage for key module scenarios.
- [x] Verify completeness and currency of `README.md` and local docs.
- [x] Lock/update verification gates for current module state.

## Widget evidence maintenance

- [ ] Refresh the live Wave 1 evidence before `refresh_policy.next_due_at`; retain audit trail, all fallback profiles, read-path no-5xx proof, SLO metrics, forum-owned correlation traces, rollback decision, approvals, waivers, and refresh-history provenance.

Fallback hardening in `contracts/evidence/fw2-fallback-static-matrix.json` keeps forum read and moderation paths available without 5xx while disabled builder capabilities return typed degraded outcomes.

FW-2 fallback evidence and FW-4 live rollout evidence require SLO response-time monitoring, rollback `<= 10 minutes` without redeploy, and the smoke `list -> open -> preview -> save_draft -> publish_dry` under `npm run verify:page-builder:consumer:forum`.
- [ ] Run `npm run verify:page-builder:consumer:forum` and `npm run verify:forum:wave-evidence-freshness` before any builder-consumer rollout or evidence refresh.
- [ ] Treat stale or incomplete evidence as a rollout block; do not start a new tenant pilot without a fresh packet.
