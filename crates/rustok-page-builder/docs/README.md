# rustok-page-builder: runtime contract

`rustok-page-builder` — reference FBA module for the visual builder.

## Purpose

The module introduces an independent builder capability contour before integration into `pages`.
This anchors FBA-first delivery and contract compatibility across host implementations.

## Scope

- independent FBA reference contour for the visual builder before integration into domain consumer modules;
- ownership of vendor-neutral payload contract (`grapesjs_v1`) and capability boundaries `preview/tree/properties/publish`;
- lifecycle/health/observability seams for rollout and safe tenant-by-tenant enablement.

## Responsibilities

- owner of the visual builder payload contract (`grapesjs_v1`) at the module level;
- lifecycle framework for rollout/health/observability in FBA terms;
- compatibility with consumer modules via contract-first integration.

## Entry points

- `src/lib.rs` — runtime metadata and permission surface;
- `src/dto.rs` — transport-neutral DTO, `PageBuilderContractMetadata::BASELINE` and typed error catalog (`validation/sanitize/runtime/feature-disabled`) for contract package without binding to transport adapters;
- `src/service.rs` — transport-neutral `PageBuilderCapabilityService`, `ReferencePageBuilderService` for compile-free provider baseline, feature-flag guard and server-side handler seam with RBAC permission checks;
- `src/transport.rs` — canonical transport bridge for GraphQL, Leptos `#[server]` and future mobile adapters on top of `AuthorizedPageBuilderHandlers::handle`;
- `src/adapters.rs` — endpoint adapter seam with framework-neutral GraphQL/Leptos payload wrappers and handler functions `handle_page_builder_graphql_endpoint` / `handle_page_builder_leptos_server_function_endpoint`, which delegate only to canonical dispatch helpers;
- `src/health.rs` — typed provider health states, degradation reasons, `ProviderHealthEvidence` and evaluator pilot SLO thresholds for release-gate evidence;
- `rustok-module.toml` — declaration of slug/entry type/ui-classification;
- `contracts/page-builder-fba-registry.json` — machine-readable registry provider/consumer versions, minimum supported consumer version and fallback profile names for anti-drift gates.
- `contracts/page-builder-flutter-wave-handoff.json` — machine-readable Flutter Wave hand-off contract for device/runtime evidence without duplicating FBA registry thresholds or control-plane toggle semantics in mobile.
- `contracts/page-builder-adapter-seams.json` — machine-readable persistence/rendering adapter-seam contract for `PageBuilderProjectStore`, `PageBuilderRenderingAdapter` and `AdapterBackedPageBuilderService`, preserving `PageBuilderCapabilityService`, `AuthorizedPageBuilderHandlers::handle`, GraphQL/Leptos endpoint wrappers and canonical DTO/envelope names.
- `PageBuilderAdapterCallEvidence` and `PageBuilderAdapterTelemetry` in `src/service.rs` capture transport-neutral evidence for host adapter operations `load_project`, `save_project` and `render_preview`: module slug, `grapesjs_v1` contract, `started/succeeded/failed` status, tenant/page/revision identifiers, correlation id and typed error markers sourced from the owner-side contract, without creating transport-local DTO.

## Integration

- `apps/server` connects the module via the `mod-page-builder` feature flag and module registry codegen;
- `rustok-pages` and other layout/content modules use the builder as consumers via the contract-first path;
- host implementations (Next/Leptos/Flutter) synchronize through the capability contract, not through a 1:1 UI mapping.


## Transport-neutral contract package

Baseline DTO package now includes `PageBuilderContractMetadata::BASELINE` with canonical provider slug `page_builder`, contract `grapesjs_v1`, `builder_contract_version = 1.0`, `consumer_min_version = 1.0` and capability set `preview/tree/properties/publish`. This is the minimal publish-ready marker for adapters: GraphQL, Leptos server functions and future mobile codegen must take capability names from contract metadata/registry, not introduce transport-local aliases.

`PageBuilderCapabilityRequest` and `PageBuilderCapabilityResponse` define a tagged-envelope for transport adapters: GraphQL resolvers, Leptos `#[server]` functions and future mobile bridge can accept a single canonical request envelope and dispatch through `AuthorizedPageBuilderHandlers::handle`. This seam holds RBAC, rollout guard and write-semantics enforcement in one place and prevents the transport layer from re-inventing capability names or local error envelopes.

The first transport bridge slice added `PageBuilderTransportKind`, `PageBuilderTransportSuccess`, `PageBuilderTransportError`, `dispatch_transport_envelope`, `dispatch_graphql_envelope` and `dispatch_leptos_server_function_envelope`. GraphQL/server-function adapters must call these dispatch helpers, then map the success/error envelope to their framework-specific result; `PageBuilderTransportError` takes `kind` and `stable_code` from `PageBuilderServiceError::kind()` / `stable_code()`, so transport does not own a separate error catalog.

The endpoint adapter seam is now established in `src/adapters.rs`: `PageBuilderGraphqlEndpointInput` and `PageBuilderLeptosServerFunctionInput` accept canonical `PageBuilderCapabilityRequest`, and `handle_page_builder_graphql_endpoint` / `handle_page_builder_leptos_server_function_endpoint` return a unified `PageBuilderEndpointResult` over `PageBuilderTransportSuccess` / `PageBuilderTransportError`. This provides real host-facing connection points for GraphQL resolvers and Leptos `#[server]` wrappers without adding framework-specific dependency in the reference module and without transport-local capability/error aliases.

## Reference provider baseline

`ReferencePageBuilderService` covers the minimal capability API baseline without vendor lock-in and without persistence side effects. The provider accepts only `grapesjs_v1`, validates `page_id`, `revision_id`, object-shaped `project_data` / `properties`, returns typed `validation` errors for contract violations and typed `sanitize` errors for forbidden preview HTML (`<script`). `preview` generates a deterministic HTML wrapper `data-rustok-page-builder="grapesjs_v1"`, `properties` echo-returns canonical node properties, and `publish` returns typed `PublishPageBuilderResult` only after contract validation. A real persistence/rendering adapter implementation can replace the reference provider behind the same `PageBuilderCapabilityService` without changing DTO, RBAC, rollout or transport bridge.

`AdapterBackedPageBuilderService` now creates `PageBuilderAdapterCallEvidence` before invoking persistence/rendering seams and passes it to `PageBuilderAdapterTelemetry` as `started`, then writes `succeeded` or `failed` outcome. Failed evidence carries `PageBuilderErrorKind` and stable code from `PageBuilderServiceError`. Default `NoopPageBuilderAdapterTelemetry` preserves the previous behavior, and host wiring can connect a recorder for the audit/observability layer around `PageBuilderProjectStore` and `PageBuilderRenderingAdapter`. Evidence is not published as a new transport response and does not change `PageBuilderCapabilityRequest/Response`.

## Provider health and SLO baseline

Machine-readable provider metadata includes health states `ready/degraded/unavailable`, degradation reasons (`capability_disabled`, `provider_unhealthy`, `sanitize_backpressure`, `publish_backlog`) and pilot SLO thresholds: `preview_p95_ms <= 1500`, `publish_p95_ms <= 3000`, `sanitize_failure_rate <= 0.01`, `runtime_error_rate <= 0.01`. The runtime code exposes the same baseline through `ProviderHealthState`, `ProviderDegradationReason`, `ProviderSloThresholds::PILOT`, `ProviderHealthSnapshot::evaluate` and `ProviderHealthEvidence::from_observations`, so Wave evidence can be formed without transport-specific adapters. Registry and Wave evidence packet gates keep these thresholds synchronized with the owner source until Wave 1 promotion.

Health evaluation rules are intentionally conservative: a preview p95 or runtime error-rate breach marks the provider as `provider_unhealthy`, a sanitize threshold breach marks `sanitize_backpressure`, a publish p95 breach marks `publish_backlog`, and a runtime error-rate above double the pilot threshold transitions state to `unavailable`; otherwise a non-empty set of degradation reasons yields `degraded`.

## Typed error catalog

The runtime provider exposes the same error semantics declared in `rustok-module.toml` and `contracts/page-builder-fba-registry.json`: `PageBuilderErrorKind::ALL` covers `validation`, `sanitize`, `runtime` and `feature-disabled`, and `PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE` anchors a stable degraded-mode code `FEATURE_DISABLED`. `PageBuilderServiceError::kind()` and `PageBuilderServiceError::stable_code()` serve as the transport-neutral bridge for GraphQL, Leptos server functions and future mobile codegen adapters, so adapters must map provider errors from these typed markers instead of local error names.

## Permission map for capabilities

Server-side capability handlers enforce a stable page permission map before delegating to the provider service. `pages:manage` remains the effective override for all builder capabilities. `PAGE_BUILDER_CAPABILITY_PERMISSIONS` publishes a serializable capability -> permission descriptor for host/codegen surfaces. `PageBuilderCapabilityPortPolicies` holds the owner-side source-lock for registry/manifest policy names, and `PAGE_BUILDER_CAPABILITY_PORT_POLICIES` publishes a serializable descriptor for port policy surfaces: `preview`, `tree` and `properties` require `PortCallPolicy::read()` deadline semantics, and `publish` requires `PortCallPolicy::write()` with deadline and idempotency key.

| Capability | Required permission | Notes |
|---|---|---|
| `preview` | `pages:read` | Read-only preview generation path. |
| `tree` | `pages:read` | Read-only node tree inspection path. |
| `properties` | `pages:update` | Editor-side property update path. |
| `publish` | `pages:publish` | Publish path; still requires `PortContext` write semantics (`idempotency_key` + deadline). |

## Fallback matrix

The runtime provider anchors baseline fallback profiles in `src/rollout.rs`; consumer modules and host adapters must use the same outcome names.

| Profile | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Disabled capabilities |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview`, `publish` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `preview`, `tree`, `properties`, `publish` |

## Verification

- `cargo test -p rustok-page-builder --lib` — baseline check of runtime metadata/contract surface;
- `cargo xtask module validate page_builder` — publish-readiness and manifest/docs contracts check;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs pages` — anti-drift check of machine-readable registry against provider/consumer manifests and owner source markers, including provider health states, degradation reasons, pilot SLO thresholds, fallback profiles, port policies, permission map and typed error catalog.
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-wave-evidence-packet.mjs` — Wave 0 evidence packet verification, including SLO thresholds/evaluation and correlation trace samples.
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-transport-bridge.mjs` — no-compile guardrail for canonical GraphQL/server-function transport bridge markers.
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-flutter-handoff.mjs` — no-compile guardrail for Flutter Wave hand-off evidence contract and mobile app-core typed error parity markers.
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs` — no-compile guardrail for persistence/rendering adapter seams; verifies `PageBuilderProjectStore`, `PageBuilderRenderingAdapter`, `ReferencePageBuilderRenderingAdapter`, `AdapterBackedPageBuilderService`, canonical entrypoints and prohibition of transport-local aliases / pages-local visual builder ownership / vendor-specific payload requirements.

## Related documents

- `docs/modules/tiptap-page-builder-implementation-plan.md` — platform rollout plan for builder-first FBA;
- `docs/modules/manifest.md` — contract for `modules.toml` / `rustok-module.toml`;
- `crates/rustok-pages/docs/implementation-plan.md` — consumer integration of `pages` with the reference builder module.
