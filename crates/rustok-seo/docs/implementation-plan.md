# Implementation plan for `rustok-seo`

Status: SEO Suite v1 is assembled as an optional platform module. Phase A–C (templates/bulk/diagnostics/schema/cross-link/image boundary) are closed at baseline. Phase D has the full D6 execution package closed; active focus has shifted to **D7–D9 (storefront parity + verification + runbooks)**.

## Execution checkpoint

- Current phase: `seo_admin_native_loco_free_transport`
- Last checkpoint: REST SEO handlers consume narrow `SeoHttpRuntime` state with explicit DB/event bus/runtime extensions handles and no longer depend on `rustok_outbox::loco`; `rustok-seo-admin` native server functions now consume `HostRuntimeContext`, read `TransactionalEventBus` and `ModuleRuntimeExtensions` through typed host handles, and no longer depend on `loco-rs` or `rustok-outbox/loco-adapter`. No-compile D8/D9 evidence seed includes concrete per-file live artifact templates for backend GraphQL/REST parity, outbox/index counters, Next runtime robots/sitemap/metadata, Leptos storefront page-context smoke, media descriptor fallback smoke and owner sign-off; it also links runbook scenarios to required live artifacts, CI attachment metadata/redaction checklists, defect triage severities and an owner sign-off state machine. `verify-seo-runtime-fixtures.mjs` validates that each required closeout artifact has capture requirements, closeout blockers and promotion guardrails without compilation. FBA consumer runtime-order smoke source-locks SEO -> media descriptor policy/alias/template order without compilation.
- Next step: gather live CI/runtime evidence packet against the deployed backend/hosts, including SEO image descriptor fallback smoke for `MediaAssetReadPort` by files `image-descriptor-in-process.json`, `provider-unavailable-omit-image-metadata.json`, `asset-unavailable-keep-existing-seo-image.json`, `relative-url-proxy-fallback.json`, `diagnostics-image-quality-before-after.json`, attach before/after counters and transition owner sign-off rows from pending to signed; do not consider D8/D9 or FBA boundary readiness closed by static evidence until then.
- Open blockers:
  - For D8, a live CI/runtime evidence packet against the deployed backend is still needed.
  - For D9, runbooks need to be supplemented with live incident evidence and owner sign-off obtained via the seeded checklist.
- Hand-off notes for next agent:
  - Do not bypass the independent `SeoTargetImageRecord` boundary and existing `SeoPageContext` contract; media descriptors are transformed at the owner/provider boundary.
  - REST/GraphQL should only be extended with additive changes in stable `v1`.
  - For the delivery tracker, maintain the invariant: one idempotency key = one actual state transition.
  - For replay mode, preserve forward-only semantics (`not_started -> repair_only -> replay_requested -> replaying -> replay_completed`) without backward transitions.
  - For Next runtime adapter, preserve semantic error mapping (`BAD_USER_INPUT` / `PERMISSION_DENIED` / `NOT_FOUND` / transport failures) and do not revert to blanket `catch {}`.
- Last updated at (UTC): 2026-07-08T00:00:00Z

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Last verification evidence:
  - `cargo fmt --all -- --check` *(pass, 2026-06-07)*
  - `cargo check -p rustok-seo-admin --config profile.dev.debug=0` *(pass, 2026-06-07)*
  - `cargo test -p rustok-seo-admin --lib --config profile.dev.debug=0` *(pass, 2026-06-07; 12 pure-core tests)*
- Scope note: module-owned UI remains infrastructure control-plane (`rustok-seo-admin` + owner-side SEO panels in `pages/product/blog/forum`); `rustok-seo-admin` now has an explicit `core/transport/ui` FFA split: `admin/src/core.rs` owns tab/busy/form policy plus effective-settings snapshot mapping via `SeoSettingsSnapshotItem` / `build_seo_settings_snapshot_items`, `admin/src/transport/mod.rs` is the facade, `admin/src/transport/native_server_adapter.rs` owns native server functions and SSR host context extraction through `HostRuntimeContext` typed DB/event-bus/runtime-extension handles with no package-local `loco-rs` or `loco-adapter` dependency, and `scripts/verify/verify-seo-admin-boundary.mjs` locks the fast boundary; transport boundary continues to evolve through GraphQL + REST `/api/seo/page-context`, `/api/seo/cross-link-suggestions`, control-plane parity endpoints and a unified GraphQL-compatible REST error envelope within Phase D.
- FBA evidence: `crates/rustok-seo/contracts/seo-fba-registry.json` declares the `seo_image_descriptor` consumer profile for `MediaAssetReadPort` / `media.asset_read.v1`, `crates/rustok-seo/contracts/evidence/seo-media-consumer-static-matrix.json` is `source_locked_pending_consumer_runtime` and mirrors consumer cases, degraded modes (`omit_image_metadata`, `keep_existing_seo_image`, `proxy_storage_relative_url`), provider fallback-smoke source, static source assertions, runtime closeout requirements, consumer runtime artifact template and drill matrix; `crates/rustok-seo/contracts/evidence/seo-media-consumer-runtime-order-smoke.json` plus `scripts/verify/verify-consumer-fba-runtime-order.mjs` lock media read-policy -> tenant parse -> owner `SeoTargetImageRecord` construction order without coupling `rustok-seo-targets` to media; status remains below `boundary_ready` until consumer runtime contract execution/fallback smoke lands.

## Scope of work

- keep `rustok-seo` as a single tenant-aware SEO runtime instead of a set of scattered SEO modules;
- synchronize metadata precedence, redirects, sitemap/robots and storefront SEO contract between server and frontend hosts;
- leave entity SEO authoring to owner modules, and use `rustok-seo-admin` only as a cross-cutting infrastructure control-plane;
- do not allow raw HTML/JSON template context, raw schema blobs and silent host-local target mappings;
- build merchant-facing automation over typed target descriptors from `rustok-seo-targets`.

## Current state

- module bootstrap, manifest wiring, migrations, permissions and local docs are connected;
- core storage uses `meta` / `meta_translations`, `seo_redirects`, `seo_revisions`, `seo_sitemap_jobs`, `seo_sitemap_files`, `seo_bulk_jobs`, `seo_bulk_job_items`, `seo_bulk_job_artifacts`;
- locale columns for SEO-related tables expanded to `VARCHAR(32)`, rollback remains forward-only and does not narrow locale;
- `SeoModuleSettings` includes typed `sitemap_submission_endpoints` with server-side normalization (`http/https`, trim, dedupe, strip fragment);
- storefront SEO read-side lives on permanent contract `SeoPageContext = route + document`;
- Rust-side SSR head rendering extracted to `rustok-seo-render`;
- `rustok-seo-admin` split into FFA layers `core/transport/ui/leptos/sections/i18n` and is not a universal entity editor;
- owner-side SEO panels embedded in `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- target extensibility goes through `rustok-seo-targets` and runtime registration providers;
- tenant templates and diagnostics are already a first-class read/control-plane layer; diagnostics covers issue aggregates, canonical redirect chains/loops, hreflang gaps, `cross_link_gap`, `missing_image_alt`, `missing_image_size`;
- read-only cross-link contract added (`seoCrossLinkSuggestions` + `/api/seo/cross-link-suggestions`) with tenant/RBAC parity;
- `SeoDocument.structured_data_blocks` is no longer raw JSON passthrough: JSON-LD is normalized into typed schema blocks (`schema_kind`, `schema_type`, legacy `kind`, `source`, payload);
- boundary contract C3 is locked through explicit media/domain descriptor conversion to owner DTO `rustok-seo-targets::SeoTargetImageRecord`;
- **open productionization gaps (Phase D):**
  - D2 closed: typed SEO event model and delivery/idempotency tracking live (`seo_event_deliveries` + outbox envelope linkage + duplicate guard);
  - D3 closed: SEO->index adapter seam live (`Seo*` events -> `index.reindex_requested`), tenant/kind-scoped triggers and bounded retry/dead-letter tracking in `seo_index_deliveries`;
  - D4 closed: REST control-plane parity complete, including GraphQL-compatible error envelope for validation/config/not_found/permission scenarios;
  - D5 closed: schema/index cursor foundation, repair path and historical replay mode complete (`run_index_repair_replay` + forward-only cursor replay transitions);
  - `apps/next-admin` API helper expanded to control-plane read/write surfaces (including index tracking/replay) and error mapping parity; owner-side observability/remediation widgets through `rustok-seo-admin-support` and wiring in `pages/product/blog/forum` are closed, open focus shifted to Next storefront runtime parity.


## Compatibility contract (Phase D freeze)

### Breaking vs non-breaking

- **Non-breaking (allowed in `v1`)**
  - additive fields in GraphQL/REST DTO;
  - new REST endpoints under `/api/seo/*` without changing existing response shapes;
  - new diagnostics issue codes and aggregates;
  - internal migration/table additions without changing current API payload contracts.
- **Breaking (prohibited in current wave)**
  - deletion/renaming of current GraphQL fields and REST endpoints;
  - changing meaning/semantics of existing enum values and source precedence;
  - changing shape of `SeoPageContext` and `SeoStructuredDataBlock` without a separate versioned contract.

### Versioning strategy

- REST/GraphQL go as stable `v1`.
- All extensions are additive.
- If an incompatible change becomes necessary, a separate `v2` track with a parallel compatibility window.

### Rollout flags (draft)

- `seo_events_enabled` — enables typed SEO event emission.
- `seo_outbox_enabled` — enables outbox relay path for SEO events.
- `seo_index_consumer_enabled` — enables SEO->index consumer adapter.
- `seo_rest_parity_enabled` — enables new REST control-plane endpoints.
- `seo_next_runtime_sitemap_enabled` — enables runtime-driven sitemap/robots in Next host.

All flags are tenant-aware, default `false` for safe staged rollout.

## Stages

### Closed

Completed core runtime, public surfaces, template, bulk-remediation, diagnostics, rich-snippet, and Phase C implementation details are maintained in [the module documentation](./README.md).

### Phase C — indexing and linking automation

- [ ] Expand Next route coverage only when real Next storefront route owners appear; the guardrail remains deferred with an explicit reason.

### Phase D — Productionization & Integration Parity

- [x] **Batch D1 — Contract freeze + scope gate**
  - [x] Lock Phase D (`D1..D9`) and execution order.
  - [x] Explicitly define breaking/non-breaking policy for GraphQL/REST/DTO.
  - [x] Lock rollout flags for event/outbox/index/API/Next parity.

- [x] **Batch D2 — Backend domain: SEO events + outbox foundation**
  - [x] Introduce typed events for: meta upsert/publish/rollback, redirect upsert/disable, sitemap generated/submitted, bulk completed/partial/failed.
  - [x] Add deterministic idempotency key (`tenant_id + target_kind + target_id + revision_or_job_id`) and scope-sensitive keys for terminal bulk states.
  - [x] Integrate emission path with `rustok-outbox` without duplicate emission in bulk loops: publish path writes `seo_event_deliveries`, links delivery to outbox envelope id and locks duplicate guard integration tests for bulk terminal events.

- [x] **Batch D3 — Indexing integration seam (SEO -> rustok-index)**
  - [x] Add consumer/adapter contract for selective invalidate/rebuild index documents.
  - [x] Add tenant/kind-scoped reindex trigger.
  - [x] Lock bounded retry + dead-letter policy for indexing failures.

- [x] **Batch D4 — GraphQL/REST parity completion**
  - [x] Add REST parity for diagnostics summary/filtering.
  - [x] Add REST for sitemap status/job detail.
  - [x] Add REST for bulk jobs list/detail/status (and preview endpoint if needed).
  - [x] Unify error envelope between GraphQL/REST (validation/config/not_found/permission).

- [x] **Batch D5 — Migrations, backfill and replay foundation**
  - [x] Add schema changes for event/outbox/index tracking.
  - [x] Prepare backfill/repair path: initial cursor/high-water mark.
  - [x] Prepare optional replay mode for historical SEO changes (`run_index_repair_replay`, `replay_historical`).
  - [x] Lock forward-only replay policy (cursor state machine + tests).
  - [x] Add control-plane transport parity for tracking/replay: GraphQL + REST.

- [x] **Batch D6 — Admin integrations (Leptos admin + next-admin + owner panels)**
  - [x] **D6.1 `rustok-seo/admin` observability surface**
    - [x] Add index delivery summary card (`pending/sent/retry/failed/dead_letter`) with tenant/target filter.
    - [x] Add cursor timeline card (`initial/high_water/last_repair/replay timestamps`) with forward-only replay badge.
    - [x] Add operator actions: `repair_only` and `repair+historical_replay` with explicit confirmation UX.
    - [x] Add failure drilldown (last_error sample + retry counters + dead-letter hints).
  - [x] **D6.2 `rustok-seo-admin-support` reusable widgets**

    - [x] Add typed remediation mapping (`issue_code -> action`), including `run_reindex` and `open_bulk_job`.
    - [x] Add shared error/permission/empty-state contract for SEO control-plane widgets.
  - [x] **D6.3 host wiring (`apps/next-admin`)**
    - [x] Connect index tracking/replay API in operator UI (using existing REST primary helper).
    - [x] Add semantic error handling for replay flows (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`).
    - [x] Add telemetry hooks (action started/success/failure) for replay/remediation operations.
  - [x] **D6.4 owner-module wiring (`pages/product/blog/forum`)**

    - [x] Check locale contract: use host effective locale, without package-local fallback chains.
  - [x] **D6.5 transport/contract hardening**
    - [x] Lock DTO limits/validation for replay input (`limit 1..500`, `target_type content|product`).
    - [x] Add anti-regression checks on idempotency key invariants after operator replay.

- [x] **Batch D7 — Storefront + Next frontend runtime parity (regrouped in Milestones A–C)**
  - [x] **Milestone A — Runtime SEO Data Plumbing (D7 foundation, large batch)**
    - [x] A.1 Add shared Next runtime adapter: REST primary + GraphQL secondary path with typed semantic error mapping (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures).
    - [x] A.2 Lock deterministic locale/route/query normalization policy in Next adapter (`routeSegment -> /modules/*`, `lang` excluded, query keys sorted).
    - [x] A.3 Unify response->metadata mapping (`canonical/hreflang/robots/OG/Twitter/verification`) and add runtime JSON-LD script extraction from `structuredDataBlocks`.
    - [x] A.4 Lock fallback-behavior evidence (module disabled / not found / permission / transport) on fixture set (`apps/next-frontend/contracts/seo/runtime-parity-fixtures.json`).
  - [x] **Milestone B — End-to-End Next Runtime Migration (D7 cutover, large batch)**
    - [x] B.1 Move `apps/next-frontend/src/app/robots.ts` to runtime-driven source with safe static fallback.
    - [x] B.2 Move `apps/next-frontend/src/app/sitemap.ts` to runtime-driven source with fallback to host-local static metadata.
    - [x] B.3 Move home `generateMetadata` to runtime `SeoPageContext` adapter semantics.
    - [x] B.4 Expand metadata smoke to at least two non-home owner routes and lock parity evidence (`product`, `blog`).
  - [x] **Milestone C — Route ownership matrix + cross-host fixture parity (D7 guardrail closure)**
    - [x] C.1 Lock route ownership matrix: owner module -> route patterns -> `target_kind` (beyond home route).
    - [x] C.2 Add unified fixture set Rust storefront vs Next host.
    - [x] C.3 Document explicit allowlist of acceptable long-tail metadata differences.

- [ ] **Batch D8 — Verification matrix and quality gates (Milestone D, heavy QA batch)**
  - [ ] D.1 Run unit coverage for normalization/validation/idempotency/replay transitions.
  - [ ] D.2 Run `rustok-seo` integration matrix (GraphQL/REST parity + outbox/index pipeline + tenant/module gating).
  - [ ] D.3 Run host integration matrix (`apps/storefront`, `apps/next-frontend`, `apps/next-admin`) with RBAC/module gating parity.
  - [x] D.4 Gather lightweight evidence packet seed (without compilation): fixture verifier + static matrix + stop criteria + live capture template.
  - [ ] D.4b Gather live CI/runtime evidence packet (backend + hosts), close high-severity parity defects.

- [ ] **Batch D9 — Docs / runbooks / readiness closeout (Milestone E, operational batch)**
  - [x] E.1 Update docs in `rustok-seo`, `rustok-seo-admin-support`, `rustok-seo-render`, `apps/storefront`, `apps/next-frontend`, `apps/next-admin`, central docs registry/index — compile-free sync matrix locked in `apps/next-frontend/contracts/seo/runtime-parity-fixtures.json`.
  - [x] E.2 Finalize baseline runbooks: `SEO event backlog stuck`, `Partial indexing failures`, `Replay/Reindex procedures` with rollback/stop criteria.
  - [x] E.2a Lock compile-free incident evidence templates for backlog, partial indexing failures and replay/reindex drills; add live artifact schema and signed-state preconditions for owner sign-off.
  - [ ] E.2b Supplement runbooks with live incident evidence after D8 backend/host runs.
  - [x] E.3 Lock owner sign-off checklist, owner closeout criteria and DoD/DoR for the next execution wave — checklist seeded in runtime parity fixture; actual signatures await live evidence.

## Remaining work (estimated 2026-06-08)

- **Phase C**: technically closed; route ownership guardrail formally tracked in Milestone C.
- **Phase D**: D1–D6 closed; D7–D9 conducted as large Milestones `A–E`.
- **Progress by Milestones**:
  - `A` — fixture evidence baseline closed;
  - `B` — runtime cutover + non-home smoke evidence baseline closed;
  - `C` — route ownership + fixture parity guardrail closed;
  - `D` — awaiting live backend/host evidence; `E` — docs/sign-off seed closed, live incident examples await D8 runtime evidence.
- **Execution focus**: D8 live evidence without local compilations, then transition D9 owner sign-off from seeded/pending to signed.
- **Total open milestone packages**: **2** (`D..E`).

## Detailed execution plan for current iteration (large packages)

### Milestone A — Runtime SEO Data Plumbing

- [x] A.1 Shared Next runtime adapter (`REST primary + GraphQL secondary path + typed errors`).
- [x] A.2 Deterministic locale/route/query normalization parity with storefront.
- [x] A.3 Unified metadata mapper + JSON-LD extraction helper.
- [x] A.4 Fallback fixtures/evidence: `module_disabled`, `not_found`, `permission_denied`, transport failures.

### Milestone B — End-to-End Next runtime migration

- [x] B.1 Runtime-driven `robots.ts`.
- [x] B.2 Runtime-driven `sitemap.ts`.
- [x] B.3 Home route `generateMetadata` + JSON-LD runtime rendering.
- [x] B.4 Minimum 2 non-home owner routes on runtime metadata adapter + smoke proof.

### Milestone C — Route ownership + cross-host fixtures

- [x] C.1 Route ownership matrix (owner -> patterns -> `target_kind`).
- [x] C.2 Unified fixture set Rust storefront vs Next host.
- [x] C.3 Explicit long-tail diff allowlist.

### Milestone D — Verification matrix execution

- [x] D.1 Lightweight fixture/static matrix seed without compilation.
  - [x] D.1a Static source assertions for Next runtime/metadata/transport, Rust renderer, Next Admin transport and Leptos storefront SEO runtime contract.
  - [x] D.1b Compile-free targeted unit coverage inventory locked in fixture: normalization/validation, replay idempotency, GraphQL page-context and storefront locale tests source-locked; actual execution remains in CI/runtime environment.
- [ ] D.2 RBAC/module gating parity checks.
  - [x] D.2a Compile-free backend/admin transport symbol guard for GraphQL/REST parity surfaces.
  - [x] D.2b Compile-free RBAC/module gating matrix for GraphQL, REST and Next fallback classification.
  - [x] D.2c Compile-free semantic error parity matrix for GraphQL validation/permission, REST envelope, Next fallback and Next Admin operator API.
- [ ] D.3 Replay/index pipeline regression checks.
  - [x] D.3a Compile-free Next Admin index tracking/replay endpoint guard.
  - [x] D.3b Compile-free replay/index idempotency invariant matrix for delivery trackers, unique transitions and forward-only replay modes.
- [x] D.4 Lightweight evidence packet seed + stop criteria.
  - [x] D.4a Compile-free host runtime entrypoint matrix for Next robots/sitemap/metadata and Leptos SSR head preflight.
  - [x] D.4a.1 Live evidence capture template for backend parity, outbox/index pipeline, Next runtime, Leptos runtime and Next Admin operator smoke.
  - [x] D.4a.2 Live artifact manifest template added: backend parity, before/after outbox/index counters, Next/Leptos host smokes, media descriptor fallback smoke and owner sign-off attachment list.
  - [x] D.4a.2.1 Concrete live artifact templates added for all required files (`backend-graphql-rest-parity.json`, `outbox-index-before-after-counters.json`, `next-runtime-robots-sitemap-metadata.json`, `leptos-storefront-page-context-smoke.json`, `media-descriptor-fallback-smoke.json`, `owner-signoff.md`) with must-capture checklist and closeout blockers without running backend/hosts.
  - [x] D.4a.3 FBA media consumer runtime artifact template added: in-process descriptor success, unavailable-provider `omit_image_metadata`, unavailable-asset `keep_existing_seo_image`, relative URL proxy fallback, diagnostics before/after counters and redaction policy.
  - [x] D.4a.4 Compile-free closeout guardrails expanded: runbook-to-artifact crosswalk, CI attachment metadata/redaction checklist, blocker/high/medium/low defect triage matrix, owner sign-off state machine and concrete template files under `contracts/seo/live-evidence/templates/` now checked by fixture verifier without compilation.
  - [ ] D.4b Live evidence packet + high-severity defect closure.

### Milestone E — Docs / runbooks / readiness closeout

- [x] E.1 Docs sync (`rustok-seo*`, host docs, central docs) — compile-free matrix covers SEO runtime docs, Next storefront, Next admin and Leptos storefront.
- [x] E.2 Baseline operational runbooks (backlog stuck / partial indexing / replay-reindex).
  - [x] E.2a Compile-free incident evidence templates for three baseline runbook scenarios.
  - [ ] E.2b Live incident examples after D8 runtime runs.
- [x] E.3 Owner sign-off checklist + DoD/DoR finalization — static checklist, closeout blockers and state-machine promotion guard ready; live signatures deferred to D8 runtime packet.

## Verification

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
- `cargo check -p rustok-outbox --tests --config profile.dev.debug=0`
- `cargo check -p rustok-index --tests --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `cargo check -p rustok-server --lib --config profile.dev.debug=0`
- `npm --prefix apps/next-admin run lint && npm --prefix apps/next-admin run typecheck`
- `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`
- `npm --prefix apps/next-frontend run lint && npm --prefix apps/next-frontend run typecheck`

## Update rules

1. When changing SEO runtime contract, first update this file.
2. When changing public/storefront surfaces, synchronize root `README.md`, local `docs/README.md` and host docs.
3. When changing module wiring, permissions or UI classification, synchronize `rustok-module.toml`, `modules.toml` and central docs.
4. When changing multilingual fallback semantics, synchronize SEO docs with `docs/architecture/i18n.md` and storefront host docs.
5. If FFA/FBA status block changes, update central readiness board `docs/modules/registry.md` in the same change.

## Quality backlog

- [ ] Close Milestone D verification matrix with real CI evidence packet; static source assertions, targeted unit inventory, integration matrix plan and live artifact manifest are already seeded in fixture verifier, but do not replace live backend/host runs.
- [ ] Lock Milestone E live incident examples and owner sign-off artifacts after D8 runtime packet.
- [ ] Update execution checkpoint after each milestone increment of Phase D.
