---
id: doc://docs/modules/tiptap-page-builder-implementation-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Rich-text (Tiptap) and GrapesJS Page Builder Implementation Plan

This document establishes a **separate implementation plan** for two related but distinct flows:
- `Tiptap`/`rt_json_v1` for rich-text scenarios in blog/forum;
- `GrapesJS`/`grapesjs_v1` for visual Page Builder as **separate FBA reference-module** with subsequent integration into `pages`.

Important: as part of FBA transition, a **standalone reference builder-module** is created first, and only then `pages` acts as consumer of this module. Integration details for `pages` remain in `crates/rustok-pages/docs/implementation-plan.md` (section `Dedicated page-builder track`), while this document establishes platform-wide implementation order and release-gate.

In FBA transition context, this track should be used as **reference "ideal FBA module"**: all new steps for `PageBuilder` are designed in FBA model (capability contracts, composable lifecycle, tenant/module feature controls, observability-first), without reverting to legacy module-scheme except explicitly marked compatibility-layers.

## 1. Goal and readiness criteria

Goal: safely migrate rich-text admin UX for blog/forum to `rt_json_v1`, separately build reference FBA-module for visual builder, and only after that connect it to `pages` without degrading RBAC, publish-pipeline, indexing and storefront-rendering.

Completion criteria:
- `rt_json_v1` used as main rich-text input format in admin for blog/forum;
- pages edited through `GrapesJS`-builder with canonical body-format `grapesjs_v1`, not through Tiptap-rich-text flow;
- legacy markdown migration conducted tenant-by-tenant with confirmed rollback-scenario;
- integration/e2e checks and observability release-gate passed;
- feature flag moved to `default-on` after stabilization.
- FBA reference-module for builder passed pilot as independent modular flow before broad inclusion in `pages`.

### Technological baseline for Page Builder (mandatory constraint)

- Base production-path for visual builder in RusTok — **open-source GrapesJS** (self-hosted).
- `grapesjs_v1` contract remains vendor-neutral: backend/runtime should not require vendor-specific payload or proprietary API.
- For Leptos/Flutter in baseline, contract-safe surfaces (preview/tree/properties/publish) on top of common backend-contract are sufficient; 1:1 visual clone of Next.js builder is not mandatory criterion for current rollout.

## 2. Phase status

- [x] **Phase 0 — Contract and backend-baseline established**
- [~] **Phase 1 — FBA reference-module extraction for builder**
- [~] **Phase 2 — Consumer integration (including `pages`) with reference-module**
- [~] **Phase 3 — Feature flags and rollout strategy**
- [ ] **Phase 4 — Legacy markdown → rt_json_v1 migration**
- [ ] **Phase 5 — Release-gate: tests, RBAC, observability**
- [ ] **Phase 6 — Pre-production smoke and pilot rollout**
- [ ] **Phase 7 — Default-on and post-release stabilization**

## 3. Implementation phases

### Phase 0 — Contract and backend-baseline (completed)

**Status:** [x] Done

- [x] Unified rich-text/page-builder contract in backend: `markdown` + `rt_json_v1` + `grapesjs_v1`.
- [x] Server-side sanitize/validation for `rt_json_v1` and schema-check for `grapesjs_v1` included in write-path.
- [x] Blog/Forum/Pages read-path returns `*_format` and `content_json` for rich payload.
- [x] Migration job `migrate_legacy_richtext` available for tenant-scoped execution.

**Output artifact:** contract ready for consumer integration.

### Phase 1 — FBA reference-module extraction for builder

**Status:** [~] In progress

- [x] Establish standalone FBA reference-flow for builder at central documentation and rollout rules level (without reverting to pages-owned implementation).
- [x] Establish capability-contracts (`preview/tree/properties/publish`) as minimally mandatory consumer surface for reference-module.
- [x] Prepare module health contract + observability baseline for reference-module (typed runtime states/reasons/SLO evaluator and Wave evidence DTO established; CI automation remains in Phase 5).
- [~] Define compatibility-perimeter for legacy payloads as temporary layer and establish sunset criteria (criteria defined, tenant-level shutdown schedule established in rollout runbook).
- [ ] Align contract parity for Next/Leptos/Flutter as consumers of reference-module at production-readiness level.

#### Runtime-baseline module health

`rustok-page-builder` now holds provider health contract not only in registry/evidence JSON, but also in typed runtime-baseline:

- states: `ready`, `degraded`, `unavailable`;
- degradation reasons: `capability_disabled`, `provider_unhealthy`, `sanitize_backpressure`, `publish_backlog`;
- pilot thresholds: `preview_p95_ms <= 1500`, `publish_p95_ms <= 3000`, `sanitize_failure_rate <= 0.01`, `runtime_error_rate <= 0.01`;
- evaluator: threshold violations give deterministic degradation reasons, and runtime error-rate above 2x threshold moves provider to `unavailable`;
- `ProviderHealthEvidence::from_observations` forms transport-neutral evidence snapshot with `pass/fail` statuses for `preview_p95_ms`, `publish_p95_ms`, `sanitize_failure_rate`, `runtime_error_rate` and `overall`.

This baseline serves as source for Wave evidence and should remain synchronized with `contracts/page-builder-fba-registry.json`.

**Phase DoD:** reference-module capable of living and rolling independently, and `pages` and other flows connect to it as consumers through stable FBA-contract.

### Phase 2 — Consumer integration (including `pages`) with reference-module

**Status:** [~] In progress

- [x] Connect `RtJsonEditor` in production CRUD-flow for blog.
- [x] Connect `ForumReplyEditor` in production CRUD-flow for forum.
- [x] Connect `PageBuilder` surfaces in `pages`-flow as consumer-flow.
- [x] Establish parity-plan for `apps/next-admin` and `apps/admin` at capability-contract level.
- [x] Align UX-handling of validation/sanitize errors in forms.
- [x] Synchronize dependency with Flutter registry/codegen plan (`docs/research/flutter.md`, anti-drift guardrail).
- [~] Establish FBA migration contract for `rustok-pages`: pages remains owner of page/menu runtime, but visual builder-domain consumed as external reference-capability layer; provider-side unified handler seam and persistence/rendering adapter seams (`PageBuilderProjectStore`, `PageBuilderRenderingAdapter`, `AdapterBackedPageBuilderService`) already ready for consumer adapters.
- [x] Move to separate runbook the procedure for enabling/disabling builder-capabilities tenant-by-tenant without rolling back entire pages runtime (see `crates/rustok-pages/docs/implementation-plan.md`, sections `Tenant switch procedure` + `FBA execution backlog`).
- [~] Consolidate capability readiness to unified FBA execution backlog for `pages` (metadata/provider contract, fallback semantics, observability correlation, CI fallback-gate).

**Phase DoD:** `pages` and adjacent flows do not directly own builder-domain, but use reference-module through stable contract.

### Cross-plan dependency note (mandatory for hand-off)

- Until backend/parity steps of this roadmap complete, Flutter team can do only contract-safe registry scaffolding.
- Any changes to mobile module contracts for page-builder must contain explicit notification about dependencies and blockers between:
  - `docs/research/flutter.md`;
  - current document;
  - `crates/rustok-pages/docs/implementation-plan.md`.

### Phase 3 — Feature flags and rollout strategy

**Status:** [~] In progress

- [~] Introduce tenant/module/form level flags (baseline-profile and naming established, rollout automation remains in backlog).
- [x] Define rollout strategy: internal → pilot → broad rollout.
- [x] Prepare tenant and module inclusion/exclusion matrix (see Phase 3.2).
- [~] Agree on operational runbook for switches (procedure and rollback-conditions established, requires owner sign-off in execution log).
- [x] Establish baseline-only rollout: OSS GrapesJS + vendor-neutral `grapesjs_v1` contract without expanding platform-contract for vendor-specifics (see Phase 3.5).
- [~] Establish FBA governance-profile for `rustok-pages` as reference-module: capability boundaries, control-plane hooks, module health contract, ownership SLA (profile and SLA baseline defined in Phase 3.6; acceptance in Phase 5).

**Phase DoD:** controlled rollout possible without redeploy.


### Phase 3.1 — Minimal feature flags profile (FBA baseline)

Mandatory baseline-profile before pilot-wave:

- `builder.enabled` — global tenant-level flag for visual builder flow access.
- `builder.preview.enabled` — permission for preview capability.
- `builder.properties.enabled` — permission for editing properties/tree.
- `builder.publish.enabled` — permission for publish through builder path.
- `builder.legacy_bridge_readonly` — enforced read-only mode for legacy block bridge.

Rules:

1. Disabling `builder.publish.enabled` should **not** break page read-path and direct publish for legacy payload.
2. Disabling `builder.enabled` moves UI to fallback-behavior (read-only + diagnostic message), without 5xx on storefront/admin list views.
3. For pilot-tenants, enabling `builder.publish.enabled=true` is forbidden if `builder.preview.enabled=false`.

### Phase 3.1.1 — Fallback matrix for capability-profiles

Unified fallback-behavior matrix synchronized with runtime helpers `rustok-page-builder::rollout` and consumer manifest `rustok-pages`. It defines minimum expected outcome for Next/Leptos/Flutter adapters without requiring 1:1 UI-clone.

| Profile | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Disabled capabilities |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview`, `publish` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `preview`, `tree`, `properties`, `publish` |

Synchronization rules:

1. When profiles change, runtime matrix in `crates/rustok-page-builder/src/rollout.rs` updated first.
2. Then consumer manifest/docs (`rustok-pages`) and this central plan are synchronized.
3. Anti-drift gate `verify-page-builder-fallback-matrix-docs.mjs`, provider runtime gate `verify-page-builder-runtime-fallback-gate.mjs` and `rustok-pages` consumer gate `verify-page-builder-pages-fallback-gate.mjs` should remain part of baseline-check until Wave 1.

### Phase 3.2 — Wave rollout matrix

Below — minimally mandatory inclusion matrix for baseline rollout.

| Wave | Tenant profile | `builder.enabled` | `preview` | `properties` | `publish` | `legacy_bridge_readonly` | Key checks |
|---|---|---:|---:|---:|---:|---:|---|
| Wave 0 (internal) | platform/synthetic | ✅ | ✅ | ✅ | ❌ | ✅ | parity payload, toggle audit trail, fallback to legacy-read |
| Wave 1 (pilot) | 1–3 low-traffic tenant | ✅ | ✅ | ✅ | ⚠️ per allowlist | ✅ | publish dry-run, RBAC parity, sanitize error-rate |
| Wave 2 (broad) | cohort tenants | ✅ | ✅ | ✅ | ✅ | ✅ (until sunset) | SLO/SLI stability, no regressions in routing/indexing |
| Wave 3 (stabilize) | default cohorts | ✅ (default-on) | ✅ | ✅ | ✅ | ❌ (after sunset) | post-rollout review, compatibility-debt closure |

Wave transition rules:

1. Transition to next wave forbidden when unclosed `P1` incidents for publish/sanitize/RBAC.
2. Each wave requires signed-off owner list: platform on-call + pages owner + runtime owner (Next/Leptos).
3. Before Wave 2, confirmed regression-check of storefront rendering for `grapesjs_v1` payload in `apps/storefront` and `apps/next-frontend` required.

### Phase 3.3 — Tenant-by-tenant switch runbook

Procedure for each tenant executed as atomic control-plane operation:

1. Take pre-check snapshot: current flags, module permissions, publish queue state.
2. Enable/disable `builder.enabled` and child capability flags in single change-set.
3. Execute smoke-checks: `preview -> properties -> publish(dry)` on test page.
4. Check observability probes: sanitize failures, publish latency, error-rate for last 15 minutes.
5. Record post-check snapshot + decision (`keep` / `rollback`) in audit trail.

6. Obtain owner sign-off per checklist: platform on-call, pages owner, runtime owner (Next/Leptos).

Execution artifacts (mandatory attach to execution log):

- pre/post toggle snapshot;
- smoke-check protocol (`preview/properties/publish(dry)`);
- metrics excerpt for 15 minutes before/after switch;
- final decision `keep`/`rollback` with owner signatures.

Immediate rollback conditions:

- runtime error-rate growth above agreed threshold;
- RBAC regression (editor/moderator/admin access diverges from policy);
- publish pipeline queue backlog exceeds baseline x2 for 10+ minutes.

SLO-check after switch:

- `preview` p95 < 1.5s;
- `publish` p95 < 3s;
- sanitize failures <= baseline + alert threshold.


### Phase 3.4 — Execution log template (mandatory minimum)

To make owner sign-off from Phase 3.3 verifiable, for each tenant change-set a unified record template is established:

```text
Tenant: <tenant_id>
Wave: <0|1|2|3>
Change-set id: <control-plane operation id>
Requested by: <owner>
Approved by: <platform on-call, pages owner, runtime owner>

Flags before:
- builder.enabled=...
- builder.preview.enabled=...
- builder.properties.enabled=...
- builder.publish.enabled=...
- builder.legacy_bridge_readonly=...

Flags after:
- builder.enabled=...
- builder.preview.enabled=...
- builder.properties.enabled=...
- builder.publish.enabled=...
- builder.legacy_bridge_readonly=...

Smoke checks:
- preview: pass/fail + latency
- properties/tree: pass/fail
- publish(dry): pass/fail + duration

Observability window (15m pre/post):
- sanitize failure rate: ...
- publish p95: ...
- runtime error-rate: ...

Decision: keep|rollback
Rollback reference: <runbook link / operation id>
Notes: <known deviations or waivers>
```

Minimum completion rules:

1. Forbidden to leave `Flags before/after` and `Decision` empty.
2. Any `waiver` requires explicit incident/ticket reference and waiver validity period.
3. For `Wave 1` and higher, storefront regression-check report reference mandatory (`apps/storefront` + `apps/next-frontend`).

### Phase 3.5 — Baseline-only rollout policy (freeze)

Within current track **strict baseline** established without expanding platform-contract:

- runtime/editor baseline: only OSS GrapesJS (self-hosted);
- transport/storage baseline: only vendor-neutral `grapesjs_v1`;
- integration policy: any vendor-specific plugins allowed only as local UI adapters without changing backend contract;
- compatibility policy: legacy bridge remains read-only until sunset, without feature growth.

Forbidden within Phase 3–5:

1. Adding new mandatory fields to `grapesjs_v1` for specific vendor.
2. Introducing control-plane flags meaningful only for vendor-specific runtime.
3. Replacing fallback semantics for legacy-read path with vendor-dependent rules.

Baseline-only policy compliance criterion:

- any new change-set in rollout contains `contract_impact: none|compatible` mark;
- for `contract_impact=compatible`, schema-diff and backward-compatibility confirmation attached.

### Phase 3.6 — FBA governance-profile for `rustok-pages` (baseline)

Minimum governance-profile before broad rollout:

| Flow | Responsible owner | SLA / reaction | Control artifact |
|---|---|---|---|
| Control-plane toggles (`builder.*`) | Platform team | rollback decision ≤ 15 min for P1 | execution log + audit trail |
| Page/menu runtime contract | Pages owners | hotfix triage ≤ 30 min for publish regression | incident ticket + runbook link |
| Runtime adapters (Next/Leptos) | Runtime owners | parity regression ack ≤ 1 business day | parity-check report |
| Observability & alerts | Platform + module owners | alert acknowledge ≤ 10 min | alert timeline + postmortem |

Mandatory governance-rules:

1. Any rollout change-set must have assigned `decision owner` and `rollback owner`.
2. Owner for control-plane and owner for runtime adapter cannot be same person in Wave 1/2.
3. For disputed cases, decision priority — platform on-call, with subsequent post-incident review fixation.

### Phase 4 — Legacy markdown → rt_json_v1 migration

**Status:** [ ] Todo

- [ ] Execute `--dry-run` migration for each tenant.
- [ ] Save reports `processed/succeeded/failed/skipped` per tenant.
- [ ] Confirm backup scope and rollback policy before production-wave.
- [ ] Conduct staged production migration execution per agreed schedule.

**Phase DoD:** target tenant-groups migrated, rollback procedurally tested.

### Phase 5 — Release-gate: tests, RBAC, observability

**Status:** [ ] Todo

- [ ] Bring to CI-ready integration/e2e scenarios for blog/forum/pages (create/update/read/publish/moderation).
- [ ] Verify RBAC enforcement for editor/moderator/admin on new routes and actions.
- [ ] Establish monitoring: sanitize-failures, error-rate, publish latency, migration metrics.
- [ ] Define alert thresholds and rollout incident response procedure.

**Phase DoD:** release-gate formalized and executed automatically.

### Phase 6 — Pre-production smoke and pilot rollout

**Status:** [ ] Todo

- [ ] Smoke-checks: create/update/read, preview/publish, index/reindex, canonical URL.
- [ ] Verify rendering parity in storefront for migrated rich-content.
- [ ] Launch pilot-wave on limited tenant list.
- [ ] Record pilot results and go/no-go decisions.

**Phase DoD:** pilot confirms stability and predictable behavior.

### Phase 7 — Default-on and post-release stabilization

**Status:** [ ] Todo

- [ ] Move flag to `default-on` for agreed tenant-groups.
- [ ] Monitor 24–72 hours key SLI/SLO and sanitization errors.
- [ ] Execute post-rollout review (risks, incidents, debt).
- [ ] Update related implementation-plan/docs per rollout results.

**Phase DoD:** feature enabled by default, operational stability confirmed.

## 4. Dependencies and related documents

- `docs/modules/overview.md` — context on module composition and brief readiness status.
- `apps/next-admin/docs/implementation-plan.md` — admin runtime integration (Next.js).
- `apps/admin/docs/implementation-plan.md` — admin runtime integration (Leptos).
- `apps/storefront/docs/implementation-plan.md` and `apps/next-frontend/docs/implementation-plan.md` — rendering parity and storefront rollout.
- `docs/architecture/api.md` and `docs/standards/rt-json-v1.md` — rich-text/page-builder payload contract.

## 5. Module focus: why Page Builder is central flow

- `blog/forum` in this plan — rich-text consumers (`rt_json_v1`), not owners of visual builder-domain.
- Visual builder-domain first lives as separate FBA reference-module; `pages` connects later as consumer of this domain.
- Any next phase-gate for builder (`feature flags`, `pilot`, `default-on`) considered incomplete without explicit status first for reference-module, then for `pages` integration.

## 6. FBA reference-module policy for builder-module

To avoid continuing implementation "in old scheme", separate builder-module established as reference module for FBA transition:

- **FBA-first delivery:** new changes in Page Builder first designed in FBA contracts/capabilities terms and only then mapped to specific host/runtime implementations.
- **Explicit compatibility perimeter:** legacy (`markdown`, block-driven pages) supported only as temporary compatibility layer with explicit sunset-plan and dependency removal metrics.
- **Control-plane alignment:** rollout, enable/disable, retry/compensation and health-check scenarios should go through standard FBA lifecycle/mechanism-practices, not through ad-hoc module toggles.
- **Parity by contract, not by framework:** parity between Next/Leptos/Flutter controlled through unified capability contract (`grapesjs_v1` + publish semantics), not UI 1:1 requirement.
- **Reference outcome:** after stabilization this module used as template for other FBA-migrations (content-like and layout-driven domains).

## 7. FBA migration blueprint using `rustok-pages` as example

Below establishes practical template for converting existing module-owned domain to FBA-model using `pages` as first consumer of reference builder-module.

### 7.1 Target role of `rustok-pages` in FBA

- `rustok-pages` does **not** own visual editor runtime as internal implementation.
- `rustok-pages` owns page/menu/visibility/publish runtime-contract and consumes builder as capability provider.
- All builder-function enabling logic goes through control-plane policies (`tenant/module/form level`) and module health signals.

### 7.2 Responsibility boundaries (ownership split)

- **Reference builder-module:** schema `grapesjs_v1`, capability endpoints (`preview/tree/properties/publish`), sanitize/validation contract, capability-health signals.
- **`rustok-pages`:** page lifecycle, publish pipeline, routing/canonical slug, channel visibility, storefront rendering guarantees.
- **Host runtimes (Next/Leptos/Flutter):** UI adapters, feature-toggle awareness, contract-layer error display without vendor-specific forks.

### 7.3 Steps for converting `pages` to FBA-consumer model

1. Establish builder-capability boundary in `rustok-pages` docs/manifest and prohibit reverting to pages-local builder ownership.
2. Add tenant-scoped capability toggles: `builder.preview`, `builder.publish`, `builder.properties` as part of rollout-profile.
3. Synchronize observability: correlation between page publish latency and builder sanitize/validation failures.
4. Verify dual-path admin integration (`native #[server]` + GraphQL selected path) without payload contract drift.
5. Move legacy blocks compatibility-layer to sunset-mode: only read/bridge path, without feature expansion.

### 7.4 FBA readiness checklist for `pages`

- [ ] `rustok-pages` runtime metadata explicitly describes external builder capability-provider.
- [ ] Rollout runbook allows partially disabling builder-capabilities without degrading page read/publish.
- [ ] CI-gate contains fallback scenarios to legacy-read path when capability-layer unavailable.
- [ ] Stores/admin UIs pass parity-check for error semantics (`validation/sanitize/runtime`).
- [ ] For legacy block-driven path, tenant-by-tenant sunset schedule approved.
- [~] For Wave 0, toggle snapshots (before/after) and audit trail in control-plane logs established (template/rules established, awaiting actual execution packets).


### 7.5 Ownership / approvals matrix

- **Platform team:** owns control-plane toggles, lifecycle hooks, rollback decision.
- **Pages module owners:** own page/menu runtime contract and storefront read guarantees.
- **Builder reference owners:** own capability API/schema (`preview/tree/properties/publish`) and sanitize policy.
- **Frontend owners (Next/Leptos/Flutter):** own adapter parity and UX fallback semantics.

Before promoting tenant to next wave, explicit confirmation from Platform + Pages owner required.



### 7.6 FBA execution plan (next 3 iterations) for `pages` as reference consumer

> Block goal: continue practical `page builder` development in FBA-model and use `rustok-pages` as template for transferring legacy module-owned domain to capability-consumer mode.

**Iteration PB-FBA-1 (contract hardening + metadata parity)**

- [~] Establish in `rustok-pages` machine-readable capability fallback matrix (`builder_off`, `preview_off`, `publish_off`) and link it with runtime error catalog (toggle profiles + degraded modes + error-catalog binding established in manifest/registry/runtime gate; cross-runtime parity evidence remains in Wave hand-off).
- [~] Close contract-parity for consumer adapters (Next/Leptos/Flutter) at level of identical error semantics, without UI 1:1 requirement (provider-side endpoint adapter seam for GraphQL/Leptos established; host wiring and mobile hand-off evidence remain in Wave gate).
- [~] Establish anti-drift checks: `contract_version` between provider/consumer should be validated in CI (baseline aggregate now includes endpoint adapter seam guard; CI wiring remains release-gate task).

**Iteration PB-FBA-2 (operability + fallback verification)**

- [~] Add mandatory fallback regression gate: disabling `builder.enabled` does not break `list/read/menu` surfaces and does not cause 5xx (provider/consumer no-compile gates exist; runtime CI regression remains Wave 1 blocker).
- [~] Link tenant-switch operations to control-plane audit trail (before/after snapshots + keep/rollback decision) — execution log template and mandatory artifacts already established in Phase 3.3/3.4, operational evidence from Wave 0 remains.
- [ ] Move unified SLO threshold checks to release-gate for wave-transitions (`preview p95`, `publish p95`, sanitize failure rate).

**Iteration PB-FBA-3 (pilot execution + sunset discipline)**

- [ ] Conduct Wave 0 for `pages` as first consumer with evidence package for each toggle profile.
- [ ] Conduct Wave 1 on 1–3 low-traffic tenant with formal go/no-go protocol.
- [ ] Establish tenant-by-tenant sunset schedule for legacy blocks bridge (only read/bridge, without write-path expansion).

**Definition of Done for section 7.6:**

- `rustok-pages` confirmed as reproducible FBA migration blueprint (contract/ops/rollout), applicable for next content/layout modules.
- All wave-transitions and rollback decisions confirmed by control artifacts, not just narrative-description.

## 8. FBA execution roadmap (continued Page Builder development)

This section establishes how to **continue Page Builder development already in FBA-model**, and how to bring `pages` to production-ready consumer-profile using it as example.

### 8.1 Builder reference module: immediate deliverables

1. **Capability runtime metadata**
   - establish in builder-module runtime metadata explicit provider-profile:
     `preview/tree/properties/publish`, health probes, degradation modes;
   - add machine-readable version of capability-contract for anti-drift checks.
   - for `rustok-pages` consumer metadata baseline already established in `crates/rustok-pages/rustok-module.toml` (`dependencies.page_builder`, `fba.builder_consumer`, `degraded_modes`, `toggle_profiles`).
2. **Control-plane handshake**
   - establish unified change-set for `builder.enabled + child flags` as atomic operation;
   - synchronize retry/compensation behavior of lifecycle hooks with control-plane runbook.
3. **Observability-first baseline**
   - link capability-layer metrics with page publish pipeline (`sanitize failures`, `publish latency`, `error_rate`);
   - add mandatory correlation-id between builder write-path and page publish events.
4. **Compatibility sunset**
   - keep legacy bridge only in read/readonly;
   - legacy write-surface expansion forbidden after Wave 0.

### 8.2 `rustok-pages` as reference consumer-module (FBA)

`rustok-pages` brought to FBA-consumer ready by four tracks:

1. **Provider contract explicitness**
   - pages runtime metadata explicitly indicates external builder provider;
   - docs/manifest prohibit pages-local re-ownership of editor runtime.
2. **Fallback semantics**
   - when `builder.enabled=false`, admin remains available in diagnostic read-only mode;
   - storefront read-path does not depend on capability endpoint availability.
3. **Typed errors / publish gating**
   - when `builder.publish.enabled=false`, publish-path returns typed runtime error, not 5xx;
   - list/read surfaces remain stable with partial disable.
4. **Operational verification**
   - tenant switch executed by `before/after` snapshot + smoke + decision log;
   - rollback policy applied without rolling back entire pages runtime.

### 8.3 FBA release gate for `builder -> pages` connection

Transition to Wave 1 allowed only if conditions simultaneously met:

- builder capability health probes stable and observable;
- `pages` passed fallback scenarios (`builder.enabled=false`, `builder.publish.enabled=false`);
- CI contains fallback regression checks for admin/storefront read paths;
- for pilot-tenant, approved owner on-call and rollback playbook exist.

### 8.4 Execution sequence (Wave 0 → Wave 1)

To remove ambiguity in team hand-off, execution established as sequence of mandatory steps:

1. **Contract freeze**
   - freeze `grapesjs_v1` fields and typed error semantics;
   - establish contract version (`builder_contract_version`) in provider and consumer (`pages`) metadata.
2. **Toggle semantics verification**
   - execute dry run for four profiles: `all_on`, `publish_off`, `preview_off`, `builder_off`;
   - for each profile save audit evidence (`before/after`, smoke output, rollback decision).
3. **Fallback CI gate**
   - enable automatic fallback-behavior checks in CI;
   - prohibit Wave 1 transition without fresh fallback evidence.
4. **Pilot readiness review**
   - joint sign-off Platform + Pages + Builder owners;
   - on-call and incident ownership agreement before pilot-tenant enablement.

### 8.6 Immediate execution backlog (next iteration)

1. **PB-FBA-1a — contract/evidence sync**
   - update `docs/modules/implementation-plans-registry.md` after Wave 0 evidence packet establishment;
   - attach links to `toggle snapshots` and `fallback gate` results for `all_on/publish_off/preview_off/builder_off`.
2. **PB-FBA-1b — error catalog binding**
   - [x] link `degraded_modes` from `rustok-module.toml` with typed runtime error catalog in `rustok-pages`;
   - [x] add anti-drift check: each degraded mode must have documented error code.
3. **PB-FBA-2a — CI fallback gate hardening**
   - bring to required-check scenarios `builder_off` and `publish_off` without 5xx on read/list surfaces;
   - establish waiver policy: temporary exceptions only with owner sign-off and expiry date.

Completion criterion for 8.6:

- machine-verifiable evidence exists that toggle semantics and fallback behavior confirmed not only by documentation, but also by CI + execution packet artifacts.

### 8.5 Artifacts mandatory for Go/No-Go

Before each wave transition, artifacts must exist:

- capability metadata snapshot (provider + consumer);
- rollout change-set with trace-id and audit trail;
- smoke report (`preview/properties/publish(dry)`);
- observability report (p95 preview/publish, sanitize failures, runtime error-rate);
- rollback confirmation note indicating responsible owner.


## 9. Practical backlog "further per plan" (Q2–Q3 2026)

Below — concrete plan continuation so teams can move **reference builder** and `rustok-pages` in parallel without blurred ownership-zones.

### 9.1 Iteration A — Capability stabilization (T+2 weeks)

**Goal:** bring reference builder-module to stable provider-contract.

- [ ] Establish `builder_contract_version` in provider metadata and add anti-drift check to CI.
- [x] Formalize baseline typed error catalog for `preview/tree/properties/publish` (`validation/sanitize/runtime/feature-disabled`) in provider/consumer metadata, provider runtime (`PageBuilderErrorKind`, `PageBuilderServiceError::kind()/stable_code()`) and runtime gate; RBAC parity remains Wave evidence.
- [x] Bring health contract to machine-readable profile (`ready/degraded/unavailable`) with degradation reason (`capability_disabled`, `provider_unhealthy`, `sanitize_backpressure`, `publish_backlog`) and anti-drift registry ↔ manifest check.
- [x] Prepare SLO-baseline for capability endpoints per pilot-tenant load class (`preview_p95_ms <= 1500`, `publish_p95_ms <= 3000`, `sanitize_failure_rate <= 0.01`, `runtime_error_rate <= 0.01`) and include sync-check in synthetic Wave evidence gate; actual tenant measurements remain Wave hand-off evidence.

**Iteration output:** builder-module has stable FBA-provider profile suitable for mass consumer onboarding.

### 9.2 Iteration B — `pages` FBA-consumer hardening (T+2–4 weeks)

**Goal:** using `pages` as example, establish canonical migration path module-owned → FBA-consumer.

- [x] In `rustok-pages` metadata establish dependency profile on external builder provider (without local ownership fallback).
- [x] Implement fallback-matrix for admin/storefront scenarios (`builder_off`, `publish_off`, `preview_off`) and confirm no 5xx in read/list.
- [x] Add publish gating contract: typed runtime error + UX guidance instead of emergency publish flow failure.
- [x] Consolidate observability correlation: one trace/correlation-id for path `builder write -> pages publish -> storefront read` established in machine-readable contract `crates/rustok-page-builder/contracts/page-builder-correlation-contract.json` and no-compile gate `verify-page-builder-correlation-evidence.mjs`; actual tenant traces remain Wave evidence.

**Iteration output:** `pages` confirmed as reference FBA-consumer, suitable as template for `content`-like modules.

### 9.3 Iteration C — Control-plane rollout readiness (T+4–6 weeks)

**Goal:** ensure safe tenant-by-tenant rollout without redeploy.

- [ ] Automate atomic toggle change-set (`builder.enabled` + child flags) through control-plane operations.
- [ ] Implement mandatory pre/post snapshot capture and audit trail attachment in runbook-procedure.
- [ ] Add rollback trigger policy to automated checks (error-rate, publish backlog, RBAC regression).
- [ ] Prepare unified on-call ownership matrix for Platform / Pages / Builder owners.

**Iteration output:** rollout-procedure reproducible and operationally ready for Wave 1.

### 9.4 Iteration D — Wave 1 pilot and broad rollout preparation (T+6–8 weeks)

**Goal:** validate production-behavior on limited tenant set.

- [ ] Conduct pilot on 1–3 low-traffic tenant with complete toggle evidence log.
- [ ] Record parity-check results between Next/Leptos/Flutter for capability semantics.
- [ ] Confirm legacy bridge remains read-only and not extended with new write-path.
- [ ] Prepare Go/No-Go decision for Wave 2 based on SLO/SLI and incident review.

**Iteration output:** broad rollout decision made based on objective release-gate data.

### 9.5 Definition of Ready for next FBA-migrations (after `pages`)

Module can follow "pages-template" only if:

- [ ] external capability-provider designated in runtime metadata;
- [ ] fallback contract exists for partial/full disable of capability-layer;
- [ ] rollout toggle semantics support atomic change-set + rollback;
- [ ] observability links capability write-path and downstream runtime effects;
- [ ] legacy compatibility has sunset-schedule with tenant-level deadlines.

### 9.6 Immediate execution-sprint (continuing per plan)

To continue without re-approval, immediate sprint established as mandatory minimum package:

1. **Builder provider contract freeze (Sprint checkpoint A1)**
   - [ ] establish `builder_contract_version` in provider/consumer metadata;
   - [ ] approve typed error semantics for `preview/properties/publish` in single changelog-entry.
2. **Pages fallback hardening (Sprint checkpoint B1)**
   - [ ] confirm `builder_off` and `publish_off` scenarios without 5xx on admin list/read;
   - [ ] add explicit UX-message for disabled publish capability.
3. **Control-plane dry run evidence (Sprint checkpoint C1)**
   - [ ] execute dry run of profiles `all_on/publish_off/preview_off/builder_off`;
   - [ ] attach `before/after` snapshots + rollback decision log.
4. **Wave 1 readiness packet (Sprint checkpoint D1)**
   - [ ] collect minimum Go/No-Go package (`metadata`, `smoke`, `observability`, `rollback note`);
   - [ ] conduct joint review Platform + Pages + Builder owners.

**Sprint exit criteria:** unified evidence-package exists for pilot Wave 1 transition without expanding document scope.


## 10. Track completion criteria (updated)

Track considered complete only when all conditions met:

- [ ] reference builder-module stabilized as independent FBA-provider;
- [ ] `rustok-pages` confirmed as production-ready FBA-consumer reference;
- [ ] rollout proceeds tenant-by-tenant through control-plane without redeploy and without critical regression;
- [ ] fallback/rollback scenarios automated and covered by CI + runbook evidence;
- [ ] migration path template published as mandatory baseline for next module migrations.


## 11. Module FBA transition program (based on `builder -> pages` template)

To ensure track not limited to only `Page Builder`, this document establishes general procedure for converting RusTok modules to FBA-architecture using `rustok-pages` as first reference consumer-case.

### 11.1 Program target coverage

Immediate wave scope includes:

- content-like modules (`blog`, `forum`, `pages`) — as priority group for capability-driven rollout;
- layout/navigation flows (`pages/menu/routing`) — as compatibility check with publish/read pipeline;
- next domains after Wave 1 stabilization — per readiness-criteria from section 9.5.

### 11.2 Unified migration pipeline for any module

Each module converted to FBA by identical sequence:

1. **Capability boundary freeze**
   - module establishes external provider/consumer boundaries in metadata/manifest;
   - hidden revert to module-local ownership for capability-domain prohibited.
2. **Control-plane onboarding**
   - capability-function enablement only through tenant-scoped toggle profile;
   - atomic change-set + mandatory rollback pathway.
3. **Fallback/compatibility hardening**
   - read/list paths must survive partial disable of capability-layer;
   - compatibility path must have sunset-deadline and owner.
4. **Observability & SLO binding**
   - correlation capability write-path ↔ downstream runtime effects;
   - mandatory SLI/SLO and alert thresholds before pilot-wave.
5. **Pilot evidence & promotion**
   - module passes Wave 0/Wave 1 with audit evidence;
   - transition to broad rollout only after owners sign-off.

### 11.3 Module queue "further per plan" after `pages`

After completing `pages` as FBA-consumer reference, next queue established as:

- **Queue A (immediately after pages):** `blog`, `forum` — bringing to full FBA-consumer model on same capability/governance profile.
- **Queue B (after Queue A):** layout-adjacent and content-index integrations tied to publish/read consistency.
- **Queue C (expansion):** remaining module-owned domains with legacy toggle/ownership debt.

For each queue, separate Go/No-Go packet required: metadata snapshot, fallback report, observability report, rollback note.

### 11.4 FBA governance checklist (mandatory for all modules)

Module considered "ready for FBA rollout" only if:

- [ ] machine-readable runtime metadata exists with explicit provider/consumer profile;
- [ ] capability-function enablement executed only through control-plane toggle policy;
- [ ] fallback semantics documented and covered by CI-checks;
- [ ] rollback executed without full rollback of adjacent runtime-flows;
- [ ] ownership matrix (Platform + Module + Frontend) approved before pilot;
- [ ] legacy compatibility has sunset milestone and tenant-level tracking.

### 11.5 Documentation currency control and anti-drift

When each module converted to FBA, mandatory updates:

1. this document (program status and module queue);
2. `docs/modules/registry.md` (current module maturity/state);
3. specific module implementation-plan (local steps and runbook);
4. release-gate evidence (CI + observability + rollback artifacts).

## 12. Immediate execution-package (May–July 2026): continued `page builder` development and `pages` FBA transfer

This block establishes concrete work package "what to do next" without re-reviewing entire roadmap.

### 12.1 Sprint 1 (until 2026-06-15): contract freeze and anti-drift

- [ ] Approve `builder_contract_version=v1` for reference builder provider and `rustok-pages` consumer metadata.
- [ ] Establish compatibility table `provider_version -> consumer_min_version` and check it in CI as hard gate.
- [ ] Establish unified typed error contract (`validation`, `sanitize`, `rbac`, `runtime`) for `preview/tree/properties/publish`.

**Sprint 1 artifacts:**
- changelog entry for contract freeze;
- CI anti-drift check report (baseline command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs`);
- updated provider/consumer metadata snapshots.

### 12.2 Sprint 2 (until 2026-06-30): `rustok-pages` fallback hardening

- [ ] Verify profiles `builder_off`, `publish_off`, `preview_off` for `apps/admin` and `apps/next-admin`.
- [ ] Confirm `list/read/menu` surfaces in `pages` give no 5xx with partial/full disable of builder capabilities.
- [ ] Establish UX-semantic for disabled publish capability (typed error + operator guidance + trace-id).

**Sprint 2 artifacts:**
- fallback regression report (admin + storefront), including baseline verify command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs`;
- incidents/alerts dry log for disable-scenarios;
- updated tenant-by-tenant switch runbook.

### 12.3 Sprint 3 (until 2026-07-15): Wave 0/Wave 1 readiness

- [~] Automate control-plane dry-run change-set for profiles `all_on/publish_off/preview_off/builder_off` (runtime seam `BuilderControlPlaneChangeSet::dry_run`, machine-readable contract `crates/rustok-page-builder/contracts/page-builder-control-plane-dry-run.json` and no-compile gate `verify-page-builder-control-plane-dry-run.mjs` added; actual tenant execution packet remains blocker).
- [~] Collect mandatory Wave 1 readiness packet: metadata, smoke, observability, rollback note (draft packet `crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json` created; no-compile guardrail `verify-page-builder-wave1-readiness-draft.mjs` now establishes pending tenant, draft change-set namespace, pending metric/sign-off markers, hold rollback reason and no waivers; actual tenant snapshots/sign-off remain Wave 1 blocker).
- [ ] Conduct joint Go/No-Go review: Platform + Builder owners + Pages owners.

**Sprint 3 artifacts:**
- audit trail with before/after snapshots;
- dry-run consistency verify report (baseline commands: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs` and `node crates/rustok-page-builder/scripts/verify/verify-page-builder-control-plane-dry-run.mjs`);
- SLO report (`preview p95`, `publish p95`, sanitize failure rate);
- signed Go/No-Go protocol for pilot tenants.
- unified baseline gate report (command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`, includes `verify-page-builder-wave1-readiness-draft.mjs`; targeted script: `npm run verify:page-builder:wave1-readiness-draft`).

### 12.4 How to scale after `pages` (further per plan)

After Sprint 3 completion, `pages` module considered reference FBA-consumer case, and pipeline from sections 9–11 transferred without changes to:

1. `blog` (Queue A) — priority on publish/read consistency and typed error parity.
2. `forum` (Queue A) — priority on moderation/publish lifecycle and fallback stability.
3. layout/index integrations (Queue B) — priority on routing/canonical/index consistency with capability degradation.

Module transition executed only with complete evidence-package from previous step (metadata + fallback + observability + rollback).

Without synchronous update of these artifacts, module not promoted to next rollout-wave.

### 12.5 Responsibility matrix and hand-off (mandatory baseline for Sprint 1–3)

To exclude implicit blockers between teams, owner-profile established for each sprint-checkpoint:

| Checkpoint | Platform team | Builder reference owners | Pages owners | Frontend owners (Next/Leptos/Flutter) |
| --- | --- | --- | --- | --- |
| Sprint 1 / A1 | approve anti-drift gate and contract registry | publish `builder_contract_version` and typed error catalog | confirm `consumer_min_version` and dependency profile | confirm adapter mapping for typed errors |
| Sprint 2 / B1 | confirm toggle policy and rollback triggers | guarantee capability health probes stability | verify `list/read/menu` fallback without 5xx | confirm UX parity for `publish_off`/`builder_off` |
| Sprint 3 / C1-D1 | conduct Go/No-Go ceremony and record decision log | attach provider health and SLO report | attach publish/read smoke and rollback note | attach parity evidence for capability semantics |

Hand-off rule: checkpoint not considered complete if at least one owner-block in table lacks confirmed artifact in release packet.

### 12.6 Minimum evidence packet template (for Wave 0/Wave 1)

For package unification between modules after `pages`, unified structure used. Machine-readable baseline established in `crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json` and checked by `verify-page-builder-wave-evidence-template.mjs`; synthetic dry-run packet for `pages` in `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json` and checked by `verify-page-builder-wave-evidence-packet.mjs`, but does not replace actual tenant evidence:

1. `metadata/`
   - provider snapshot (`builder_contract_version`, health profile, degraded modes);
   - consumer snapshot (`dependency profile`, fallback matrix, toggle profiles).
2. `fallback/`
   - results for `all_on/publish_off/preview_off/builder_off`;
   - confirmation of no 5xx in `admin list/read` and `storefront read`.
3. `observability/`
   - `preview p95`, `publish p95`, sanitize failure rate, runtime error rate;
   - correlation trace examples `builder write -> pages publish -> storefront read`.
4. `rollback/`
   - rollback decision log (`keep` / `rollback`) with reason;
   - owner on-call confirmation and timestamp.

Minimum standard: without complete packet template, module cannot transition from Wave 0 to Wave 1. Template and synthetic packet gate should remain part of aggregate baseline gate `verify-page-builder-fba-baseline.mjs`; Wave 1 transition requires replacing synthetic snapshots with actual before/after artifacts from tenant dry-run.

### 12.7 Next practical step "right now" (next 10 working days)

For team to continue work without additional re-planning, minimum starter package established for next 10 working days:

1. **Contract registry update**
   - [x] create/update machine-readable record `builder_contract_version=1.0` for provider and `consumer_min_version=1.0` for `rustok-pages`;
   - [x] add link to record in `docs/modules/registry.md` and local implementation-plan `crates/rustok-pages/docs/implementation-plan.md`.
2. **Fallback smoke baseline**
   - [~] execute smoke for profiles `all_on`, `publish_off`, `preview_off`, `builder_off` on one internal tenant (contract/source gate ready, actual tenant smoke outputs still pending);
   - [ ] attach brief report with facts for `admin list/read`, `storefront read`, `publish(dry)`.
3. **Observability wiring check**
   - [x] confirm correlation-id presence in chain `builder write -> pages publish -> storefront read` at Wave 0/Wave 1 evidence packets and source/doc markers level (`verify-page-builder-correlation-evidence.mjs`);
   - [ ] establish baseline-values `preview p95`, `publish p95`, sanitize failure rate.
4. **Go/No-Go prep draft**
   - [x] prepare draft Wave 1 readiness packet per template 12.6 (`crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json`, check `verify-page-builder-wave1-readiness-draft.mjs`);
   - [ ] conduct asynchronous review by Platform/Builder/Pages/Frontend owners.

**Exit criteria (next 10 working days):**
- valid contract registry snapshot exists;
- fallback smoke evidence exists minimum for `all_on/publish_off/preview_off/builder_off`;
- observability baseline exists with correlation examples;
- readiness packet draft exists with unclosed risks and owner-assignments.

### 12.8 Risk register for Sprint 1–3 and escalation rules

To ensure "further per plan" does not become narrative-only tracking, mandatory risk register established:

| Risk ID | Risk description | Trigger | Mitigation | Escalation owner |
| --- | --- | --- | --- | --- |
| PB-FBA-R1 | anti-drift between provider/consumer metadata | incompatible `builder_contract_version`/`consumer_min_version` | hard CI gate + rollback to last compatible pair | Platform team |
| PB-FBA-R2 | fallback regression in `pages` read surfaces | 5xx or timeout with `builder_off`/`publish_off` | Wave promotion block + hotfix fallback matrix | Pages owners |
| PB-FBA-R3 | capability health degradation under pilot load | `preview/publish` p95 above SLO or sanitize failures growth | limit rollout cohort + tuning + repeat smoke | Builder reference owners |
| PB-FBA-R4 | UX drift between Next/Leptos/Flutter adapters | differing typed error semantics | parity review checkpoint + unified error mapping table | Frontend owners |

**Escalation SLA:**
- critical risks (`R1`, `R2`) escalated within 30 minutes from detection;
- degradation risks (`R3`, `R4`) — within 1 business day with mandatory remediation plan;
- without closed mitigation item, module not promoted to next rollout-wave.

### 12.9 Execution cadence and DoD by waves (to continue without re-interpretation)

#### Weekly cadence (until Wave 1 completion)

- **Monday (plan sync, 30 min):**
  - status update for checkpoints `A1/B1/C1-D1`;
  - open risks `PB-FBA-R1..R4` check and owner/action assignment.
- **Wednesday (evidence sync, 30 min):**
  - verification that `metadata/fallback/observability/rollback` package populated with actual artifacts;
  - drift-remarks recording between provider/consumer metadata.
- **Friday (promotion review, 30 min):**
  - `keep/rollback/hold` decision for current tenant cohort;
  - go/no-go status and next wave blockers update.

#### Definition of Done for Wave 0 -> Wave 1 transition

Transition allowed only when simultaneously fulfilled:

1. **Contract integrity**
   - [x] `builder_contract_version` and `consumer_min_version` confirmed by anti-drift gate without waiver (`verify-page-builder-contract-registry.mjs`; CI aggregate gate updated).
2. **Fallback integrity**
   - [ ] `all_on/publish_off/preview_off/builder_off` verified and no 5xx in `admin list/read` + `storefront read`.
3. **Operational integrity**
   - [ ] complete audit trail exists (`before/after`, smoke, decision log) for toggle change-set.
4. **Observability integrity**
   - [~] SLO-boundaries confirmed (`preview p95`, `publish p95`, sanitize failure rate) and correlation trace examples exist: static/draft examples established by no-compile gate, actual tenant SLO remain Wave 1 blocker.
5. **Ownership integrity**
   - [ ] explicit sign-off exists from Platform + Builder + Pages + Frontend owners.

If any item not closed, wave status remains `hold`, and module not scaled to `blog/forum` queues.

### 12.10 What we consider "continued per plan" by end of July 2026

To establish measurable result, by **2026-07-31** minimum outcome expected:

- [ ] `pages` passed Wave 0 with complete evidence packet and without blocking `R1/R2`.
- [~] Wave 1 readiness packet prepared as draft and awaits actual tenant snapshots/SLO/sign-off from owner-groups; draft-only invariants established by no-compile gate, so accidental hold-markers removal blocked until real rollout evidence.
- [ ] For `blog` and `forum`, starter migration backlog created by same template (`contract/fallback/observability/rollback`).
- [ ] In `docs/modules/registry.md`, current maturity-state for `builder/pages` track reflected.

This outcome is checkpoint for decision about transition to broad rollout (Wave 2) in next planning cycle.

## 13. Forum UI as widget-driven consumer of Page Builder (phpFox-like scenario)

Below establishes target interpretation if forum UI assembled from page-builder "building blocks" (widgets/blocks), as in phpFox-like approach.

### 13.1 What changes in `forum` role in this track

- `rustok-forum` remains owner of forum domain (topics/replies/moderation/policies), but UI-composition of forum pages transitions to capability-consumer mode through builder widgets.
- Builder in this case acts as layout/composition layer, not forum runtime replacement.
- `forum` does not receive pages-local ownership of editor runtime; it consumes same reference provider-contract (`preview/tree/properties/publish` + typed errors).

### 13.2 Minimum widget contract for forum-builder integration

For rollout without vendor-lock, mandatory baseline:

1. `widget_type` (machine-readable identifier, for example `forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`);
2. `data_contract_version` (widget input data version, validated by anti-drift gate);
3. `props_schema` (validatable JSON schema for UI-settings);
4. `capability_requirements` (`preview`, `publish`, `moderation_view` if needed);
5. `fallback_mode` (`readonly`, `hidden`, `degraded`) with partial disable of builder capabilities.

### 13.3 Responsibility boundaries (mandatory)

- **Forum owners:** domain data, moderation semantics, ACL/RBAC checks, query contracts.
- **Builder owners:** widget rendering host, layout tree, publish pipeline integration, typed error surfacing.
- **Frontend owners:** adapter parity (Next/Leptos/Flutter), UX fallback when individual widgets unavailable.
- **Platform team:** tenant toggle policy, rollout/rollback governance, observability SLO gates.

### 13.4 Rollout constraints (to not break forum runtime)

- Forbidden to move forum domain-logic into widget layer; widgets only compose and display already contracted forum capabilities.
- With `builder_off`, forum read-path must remain available through baseline forum routes (without 5xx).
- For Wave 1, parity-check required: identical typed error semantics for forum widgets on Next/Leptos/Flutter.
- Any widget props expansion must go through versioned `data_contract_version` and CI anti-drift check.

### 13.5 Forum widgets implementation queue after `pages`

- [x] **FW-1 (contract freeze):** widget catalog v1 (`topic_list/topic_detail/reply_stream`), `data_contract_version`/compatibility matrix and typed error mapping established as machine-readable contract (manifest + REST/GraphQL catalog surface), without rollout activation until `P5`.
- [~] **FW-2 (fallback hardening):** manifest/doc static gate `npm run verify:page-builder:consumer:forum` establishes `builder_off/publish_off`, widget fallback modes `readonly`/`degraded`/`hidden` and validates `crates/rustok-forum/contracts/evidence/fw2-fallback-static-matrix.json` with no-5xx read/moderation source-marker assertions; runtime smoke after `P5`.
- [ ] **FW-3 (pilot):** enable 1–2 low-traffic tenant with evidence packet (metadata/fallback/observability/rollback).
- [ ] **FW-4 (promotion):** expand rollout only after owner sign-off and SLO stability 24–72h.

## 14. Execution order update: "no loose ends" (single critical path)

To proceed through plan without accumulating parallel unclosed branches, mandatory execution order established below.

### 14.1 Prioritization rule

- Until `Section 12 / Sprint 1–3` closure, new scope-expansions (including FW-1..FW-4 for forum widgets) do not start in delivery, allowed only as design-ready backlog.
- Any task not affecting current wave-gate (`Wave 0 -> Wave 1`) gets `deferred` status.
- Consider "loose end" any unclosed task from current checkpoint without artifact in evidence packet.

### 14.2 New order (reordered execution queue)

1. **P0 — Contract freeze + anti-drift (mandatory start)**
   - close `builder_contract_version` + `consumer_min_version`;
   - enable CI anti-drift gate without waiver.
2. **P1 — Fallback hardening (before any pilot steps)**
   - confirm `all_on/publish_off/preview_off/builder_off` without 5xx;
   - establish typed error parity for Next/Leptos/Flutter.
3. **P2 — Control-plane operability**
   - dry-run atomic toggle change-set;
   - mandatory before/after snapshots + decision log.
4. **P3 — Observability & SLO gate**
   - establish `preview p95`, `publish p95`, sanitize failure rate;
   - confirm correlation-id chain `builder write -> pages publish -> storefront read`.
5. **P4 — Wave 0 execution**
   - execute internal wave and collect complete evidence packet.
6. **P5 — Wave 1 readiness / Go-NoGo**
   - joint owner sign-off Platform + Builder + Pages + Frontend;
   - only after sign-off, forum FW-1 activation in delivery allowed.

### 14.3 Explicit deferred-list until P5 closure

- FW-1/FW-2/FW-3/FW-4 for forum widgets — **deferred** (except contract clarification in documentation).
- Rollout expansion beyond pilot cohort — **deferred**.
- Broad rollout / default-on scenarios — **deferred**.

### 14.4 "No loose ends" criterion for next item transition

Transition from `P(n)` to `P(n+1)` allowed only if simultaneously fulfilled:

- [ ] all current step checklist-items closed;
- [ ] related evidence artifact exists (metadata/fallback/observability/rollback);
- [ ] no open critical risks (`PB-FBA-R1`, `PB-FBA-R2`);
- [ ] `next action` in registry and local implementation-plan synchronized with new step.

If at least one condition not met — step remains `in_progress`, new directions not opened.


## 8. Continued page builder development (current sprint for `rustok-pages`)

This block establishes next practical step after Phase B closure in `rustok-pages`:
moving consumer-flow of page builder to CI-verifiable FBA baseline for Wave 0.

### 8.1 Sprint objective (PB-FBA-1)

- Close typed fallback matrix for profiles `builder_off`, `preview_off`, `publish_off`.
- Establish unified error catalog (`validation`, `sanitize`, `runtime`, `feature-disabled`)
  without drift between `#[server]`, GraphQL and UI adapters.
- Add CI fallback-gate for profiles `all_on`, `publish_off`,
  `preview_off` and `builder_off`.
- Form Wave 0 evidence package: toggle snapshots + smoke output +
  observability snapshot + decision note (`keep/rollback`).

### 8.2 Delivery slices (step-by-step execution)

1. **Contract slice:** add machine-readable mapping of fallback-profiles in runtime metadata
   `rustok-pages` and synchronize with module docs.
2. **Error semantics slice:** bring payload typed errors to one catalog key-space
   for `preview/properties/publish` capability endpoints.
3. **Verification slice:** expand module test gate with target checks
   `all_on/publish_off/preview_off/builder_off` without list/read degradation.
4. **Operability slice:** format unified evidence-template for Wave 0 and link
   to control-plane audit trail.

### 8.3 Exit criteria for Wave 1 hand-off

- CI fallback regression checks stably green on current commit.
- RBAC parity confirmed for `editor/moderator/admin` in builder-related scenarios.
- Rollback toggle execution fits <=10 minutes without redeploying `pages` runtime.
- Legacy blocks path established as read/bridge-only (without write-surface expansion).

### 8.4 Dependency sync note (mandatory synchronization)

For each completed slice, mandatory simultaneous update:

- `crates/rustok-pages/docs/implementation-plan.md` (execution checkpoint + backlog);
- current document (phase-level platform track);
- `docs/research/flutter.md` (explicit mobile contract scaffolding dependency status).

Unsynchronized changes considered release-blocker for pilot-wave.

### 8.5 Execution backlog (next 2 weeks, without scope expansion)

Current sprint status: `in_progress` (focus only on `P0 -> P3` from section 14.2).

#### Week 1 — close P0/P1

- [x] **PB-FBA-1A / Contract freeze:**
  - [x] establish `builder_contract_version=1.0` and `consumer_min_version=1.0` in machine-readable registry `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`;
  - [x] attach anti-drift diff-check in baseline gate (`verify-page-builder-contract-registry.mjs`, aggregate `verify-page-builder-fba-baseline.mjs`, fail-fast for incompatibility).
- [~] **PB-FBA-1B / Fallback hardening:**
  - [x] confirm service-level smoke-profiles `all_on/publish_off/preview_off/builder_off` without degrading `pages` read/list through `pages_builder_fallback_*` gate;
  - [x] attach admin/storefront host-helper evidence without read/list degradation and without builder capability requirement on storefront render;
  - [x] link `degraded_modes` with typed error catalog (`FEATURE_DISABLED`) in provider/consumer metadata, FBA registry and runtime anti-drift gate;
  - [x] establish Next Admin typed-error parity (`validation/sanitize/runtime/feature-disabled`) and operator guidance through static baseline gate;
  - [x] establish Leptos admin typed-error parity and localized operator guidance through static baseline gate;
  - [x] establish Flutter app-core typed-error parity and operator guidance through static baseline gate;
  - [~] collect device/runtime evidence packet for Flutter adapters in Wave hand-off (machine-readable hand-off contract and no-compile gate established; actual device/runtime packet remains Wave hand-off task).


#### PB-FBA-1B Flutter Wave hand-off contract

To close Flutter parity without prematurely opening pilot rollout, separate machine-readable contract `crates/rustok-page-builder/contracts/page-builder-flutter-wave-handoff.json` established. It holds boundary: Flutter provides only device/runtime evidence for shared app-core mapper (`validation/sanitize/runtime/feature-disabled`, `FEATURE_DISABLED`, operator guidance and no local toggle policy), but does not duplicate FBA registry thresholds or control-plane toggle semantics in mobile registry. Gate `verify-page-builder-flutter-handoff.mjs` added to aggregate no-compile baseline, so Wave 1 promotion remains blocked until actual device/runtime packet and owner approvals.

#### Week 2 — close P2/P3

- [~] **PB-FBA-1C / Control-plane operability:**
  - [x] establish machine-readable evidence template for metadata/control-plane/fallback/observability/rollback/approvals;
  - [x] establish synthetic Wave 0 dry-run packet for all baseline toggle profiles as gate form/semantics;
  - [ ] conduct real dry-run toggle change-set (tenant internal), save actual `before/after` snapshots;
  - [ ] format real decision log (`keep|rollback`) with owner sign-off.
- [~] **PB-FBA-1D / Observability baseline:**
  - [x] establish synthetic Wave 0 baseline-metrics and SLO thresholds/evaluation (`preview p95`, `publish p95`, `sanitize failure rate`, `runtime error rate`) in evidence packet; actual tenant-metrics remain Wave hand-off task.
  - [x] attach minimum 2 synthetic correlation traces (`builder write -> pages publish -> storefront read`) and include `trace_samples` check in evidence gate; actual trace examples remain Wave hand-off task.

#### Artifacts mandatory for checkpoint update

1. `metadata snapshot` (provider/consumer versions + fallback profile mapping): `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`;
2. `fallback smoke report` (`all_on`, `publish_off`, `preview_off`, `builder_off`): service-level gate `cargo test -p rustok-pages --test page_service_kind_guard pages_builder_fallback`, admin/storefront host-helper static checks inside `verify-page-builder-pages-fallback-gate.mjs`;
3. `toggle audit log` (change-set id, before/after, decision);
4. `observability snapshot` (p95/error-rate/sanitize + minimum 2 `trace_samples`; synthetic baseline already checked by gate, actual tenant traces needed for Wave hand-off).

#### Hard constraints for backlog period

- Forbidden to open delivery for `FW-1..FW-4` until complete `P5` closure (section 14.2).
- Any waiver for anti-drift or fallback-check automatically sets Wave 1 readiness status to `hold`; current PB-FBA-1A anti-drift gate must pass without waiver.
- Any change in builder-contract without synchronous update:
  - `crates/rustok-pages/docs/implementation-plan.md`;
  - `docs/modules/registry.md`;
  - `docs/research/flutter.md`;
  considered release-blocker.


### Guardrails for adapter seams

- Adapter seams in `rustok-page-builder` should not introduce transport-local capability/error aliases, return pages-local ownership of visual builder, or require vendor-specific project payloads.
