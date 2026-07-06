# Implementation plan for `rustok-page-builder` (FBA reference module)

## Context

`rustok-page-builder` is created as an independent FBA reference module.
The first stage is to stabilize capability contracts and runtime seams,
after which the module is connected as a consumer-dependency in `rustok-pages`.

## Stages

- [x] Phase 0 — bootstrap module contract (`Cargo.toml`, `rustok-module.toml`, `RusToKModule`).
- [x] Phase 1 — capability API baseline (`preview/tree/properties/publish`) without vendor lock-in.
- [x] Phase 2 — observability and module health contract baseline.
- [ ] Phase 3 — integration contract for `pages` as consumer.
- [ ] Phase 4 — rollout controls (feature flags / tenant gates / pilot).

## Current state

- runtime module scaffold is complete;
- module manifest and docs contracts are created;
- machine-readable FBA registry (`contracts/page-builder-fba-registry.json`) locks provider version, `consumer_min_version`, consumer contract versions, fallback profile set, provider health states, degradation reasons and pilot SLO thresholds for anti-drift gate, synchronized with owner source markers in `rollout.rs` and `health.rs`;
- server feature wiring (`mod-page-builder`) is connected;
- typed provider health/SLO evaluator added to runtime baseline for Wave evidence;
- transport-neutral DTO metadata (`PageBuilderContractMetadata::BASELINE`), typed provider error catalog (`PageBuilderErrorKind`, `PAGE_BUILDER_ERROR_CATALOG`, `PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE`) and typed Wave health evidence (`ProviderHealthEvidence`) are created as publish-ready contract markers;
- transport-neutral tagged request/response envelope and `AuthorizedPageBuilderHandlers::handle` added as entrypoint seam for future GraphQL/server-function adapters;
- transport bridge slice added `src/transport.rs` with `dispatch_graphql_envelope` / `dispatch_leptos_server_function_envelope` and canonical success/error envelope over `AuthorizedPageBuilderHandlers::handle`;
- endpoint adapter seam added `src/adapters.rs` with GraphQL/Leptos payload wrappers and host-facing handler functions over canonical dispatch helpers;
- machine-readable correlation contract `contracts/page-builder-correlation-contract.json` locks evidence chain `builder write -> pages publish -> storefront read` and source markers for no-compile gate;
- capability handlers have reference-provider baseline (`ReferencePageBuilderService`) for `preview/tree/properties/publish` with contract validation, sanitize guard and deterministic typed responses;
- persistence/rendering extension slice added through `PageBuilderProjectStore`, `PageBuilderRenderingAdapter`, `ReferencePageBuilderRenderingAdapter` and `AdapterBackedPageBuilderService`, so host adapters can connect storage/rendering without changing DTO, `PageBuilderCapabilityService`, `AuthorizedPageBuilderHandlers::handle` or GraphQL/Leptos endpoint wrappers;
- adapter lifecycle evidence slice added `PageBuilderAdapterOperation`, `PageBuilderAdapterCallEvidence`, `PageBuilderAdapterTelemetry` and default `NoopPageBuilderAdapterTelemetry` for typed audit/observability markers `load_project`, `save_project` and `render_preview` over `PortContext` without changing capability DTO or transport envelopes; evidence now writes `started/succeeded/failed` outcome and carries `PageBuilderErrorKind` + stable code for failed adapter calls;
- permission descriptor slice locked serializable `PAGE_BUILDER_CAPABILITY_PERMISSIONS`, so the capability -> permission map from registry/manifest is accessible to host/codegen surfaces from the owner crate;
- read capability policy slice locked `PageBuilderCapabilityPortPolicies`, serializable `PAGE_BUILDER_CAPABILITY_PORT_POLICIES`, `PortCallPolicy::read()` for `preview`, `tree` and `properties`, so all capability handlers now require deadline semantics, while `publish` preserves write deadline + idempotency enforcement;
- Control-plane dry run evidence locked in `contracts/page-builder-control-plane-dry-run.json`: atomic change-set for `builder.enabled` and child flags, mandatory profiles `all_on/publish_off/preview_off/builder_off`, before/after snapshots, waiver policy and read-surface guarantees.


## FFA/FBA status

- FFA status: `not_started` (the reference provider does not yet have a module-owned UI)
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Evidence:
  - module exists as an independent reference provider for `preview/tree/properties/publish`;
  - machine-readable registry locks provider/consumer versions, fallback profiles, health states, degradation reasons and SLO thresholds, and the contract-registry guardrail source-locks these values in `BuilderToggleProfile` / `fallback_matrix` and `ProviderHealthState` / `ProviderDegradationReason` / `ProviderSloThresholds::PILOT`;
  - baseline verification gates cover provider/consumer anti-drift, Wave evidence template, synthetic Wave 0 packet, Wave 1 readiness draft and correlation evidence `builder write -> pages publish -> storefront read`;
  - runtime health contract locks `ready/degraded/unavailable`, degradation reasons, pilot SLO thresholds and typed SLO evaluation evidence in code;
  - migration slice moved `PageBuilderCapabilityService` to explicit `PortContext`, shared `PageBuilderCapabilityPortPolicies` / `PAGE_BUILDER_CAPABILITY_PORT_POLICIES`, `PortCallPolicy::read()` for `preview/tree/properties` and `PortCallPolicy::write()` for `publish` without changing DTO contract.
  - server-side handler seam added permission map `preview/tree -> pages:read`, `properties -> pages:update`, `publish -> pages:publish` with `pages:manage` override, serializable `PAGE_BUILDER_CAPABILITY_PERMISSIONS` and registry/manifest anti-drift check.
  - provider runtime now exposes typed error catalog `validation/sanitize/runtime/feature-disabled`, source-locked through `PageBuilderErrorKind` / `PAGE_BUILDER_ERROR_CATALOG`, and stable degraded-mode code `FEATURE_DISABLED` for transport adapters.
  - transport bridge slice locks canonical dispatch helpers for GraphQL and Leptos server-function adapters; no-compile guardrail `verify-page-builder-transport-bridge.mjs` checks that adapters do not bypass `AuthorizedPageBuilderHandlers::handle` and typed error mapping.
  - endpoint adapter seam locks framework-neutral GraphQL/Leptos endpoint payloads and `handle_page_builder_graphql_endpoint` / `handle_page_builder_leptos_server_function_endpoint`; no-compile guardrail `verify-page-builder-endpoint-adapters.mjs` keeps endpoint wrappers on canonical request/response envelopes.
  - capability API baseline is closed with a reference provider without persistence side effects: `preview` renders a deterministic wrapper, `properties` returns canonical node properties, `publish` returns typed publish result after `grapesjs_v1` validation, and forbidden preview HTML is mapped to typed `sanitize` error.
  - Control-plane dry run evidence contract and runtime `BuilderControlPlaneChangeSet::dry_run` lock atomic toggle change-set, mandatory profile snapshots, rollback decision marker and waiver policy; aggregate no-compile baseline includes `verify-page-builder-control-plane-dry-run.mjs`.
  - adapter seam contract `contracts/page-builder-adapter-seams.json` and runtime traits `PageBuilderProjectStore` / `PageBuilderRenderingAdapter` lock extension-point for persistence/rendering without transport-local capability aliases, transport-local error kind aliases, pages-local visual builder ownership or vendor-specific required project payloads.
  - adapter operation evidence (`PageBuilderAdapterCallEvidence` + `PageBuilderAdapterTelemetry`) locks `module_slug`, `grapesjs_v1` contract, operation, `started/succeeded/failed` status, tenant/page/revision ids, correlation id and typed failure markers around host persistence/rendering adapters, so the audit/observability layer remains on the owner-side FBA contract, not a transport-local convention.
  - runtime-order smoke `contracts/evidence/page-builder-orchestrator-runtime-order-smoke.json` and the common no-compile gate `scripts/verify/verify-orchestrator-fba-runtime-order.mjs` lock the order of capability flag -> `PortCallPolicy` -> owner service call, authorization -> service call, fallback profiles and GraphQL/Leptos endpoint dispatch seams without running Cargo.
- Last verified at (UTC): 2026-06-30T00:00:00Z
- Owner: `rustok-page-builder` module team

## Immediate next steps

1. Connect host GraphQL resolvers and Leptos `#[server]` wrappers to `handle_page_builder_graphql_endpoint` / `handle_page_builder_leptos_server_function_endpoint`, preserving `PageBuilderCapabilityRequest/Response`, `PageBuilderServiceError::kind()` and `stable_code()` as the canonical transport bridge without transport-local capability/error aliases.
2. Replace draft dry-run snapshots with actual tenant evidence packet without waivers before Wave 1 promotion.
3. Keep `verify-page-builder-transport-bridge.mjs`, `verify-page-builder-endpoint-adapters.mjs`, `verify-page-builder-control-plane-dry-run.mjs`, `verify-page-builder-contract-registry.mjs`, `verify-page-builder-wave-evidence-packet.mjs`, `verify-page-builder-wave1-readiness-draft.mjs`, `verify-page-builder-correlation-evidence.mjs`, `verify-page-builder-adapter-seams.mjs` and aggregate `verify-page-builder-fba-baseline.mjs` in the baseline gate for provider/consumer anti-drift, health/SLO/fallback source sync, permission-map/port-policy/error-catalog sync, Wave evidence form and correlation chain `builder write -> pages publish -> storefront read`.
4. Connect specific host persistence/rendering adapter to `AdapterBackedPageBuilderService` in server/consumer wiring, preserving `CapabilityGuardedService` for rollout flags and `PortCallPolicy::write()` enforcement.
5. Describe sunset path for legacy block-driven compatibility.

## Scope of work

- runtime capability contract (`preview/tree/properties/publish`);
- permission/RBAC enforcement for builder lifecycle actions;
- observability and health contracts for control-plane rollout;
- consumer-integration protocol for `rustok-pages` and other modules.

## Verification

- `cargo xtask module validate page_builder`
- `cargo test -p rustok-page-builder --lib`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs pages` (no-compile baseline gate for contract/evidence/fallback source markers; does not replace Cargo checks when compilations are allowed)

## Update rules

- when changing capability contracts, update `docs/README.md` and this plan simultaneously;
- when changing rollout/ownership, synchronize `docs/modules/tiptap-page-builder-implementation-plan.md`;
- do not keep historical changelog: maintain only the current state of stages and upcoming work.

## Related documents

- `docs/modules/tiptap-page-builder-implementation-plan.md`
- `docs/modules/manifest.md`
- `crates/rustok-page-builder/docs/README.md`
- `crates/rustok-pages/docs/implementation-plan.md`
