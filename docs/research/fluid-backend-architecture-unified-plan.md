---
id: doc://docs/research/fluid-backend-architecture-unified-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Unified Fluid Backend Architecture (FBA) Implementation Plan for RusTok

This document is the **single current FBA implementation plan** for RusTok.
It replaces scattered planning materials and defines the mandatory stage sequence.

Related concept document: [Fluid Backend Architecture for RusTok](./fluid-backend-architecture.md).

The [Fluid Frontend Architecture (FFA)](./fluid-frontend-architecture.md) and [Fluid Backend Architecture (FBA)](./fluid-backend-architecture.md) bundle gives RusTok module portability between embedded and headless/remote profiles without rewriting core logic.


## 0) Visual Admin Context (UI parity)

Below are illustrations of two admin runtime variants that must preserve the same
business semantics and navigation contract under FFA+FBA.

### Leptos variant (SSR-first)

![Leptos Admin Dashboard](https://github.com/user-attachments/assets/leptos-admin-dashboard)

*Description:* dark SSR-first admin dashboard with blocks `Total users / Content nodes / Orders / Revenue snapshot`,
`Recent activity` block and `Enabled modules` widget. This is the reference for the Leptos-host profile
(`apps/admin`) and module-owned UI route contract.

### Next variant (headless/runtime parity)

![Next Admin Dashboard](https://github.com/user-attachments/assets/next-admin-dashboard)

*Description:* Next-host variant with the same semantic set: metric dashboard, activity,
operator actions and modular navigation. This is the reference for `apps/next-admin`, which must
maintain parity with the Leptos variant in data, roles and scenarios.

> Note: visual style may differ, but the FFA+FBA contract requires stability
> of domain scenarios, access rights, route/query semantics and backend orchestration behavior.

---

## 1) Purpose and Boundaries

## 1.1 Purpose

Move individual module boundaries into a remote execution profile (e.g., gRPC/async worker) **without rewriting domain/application logic**.

## 1.2 FBA Architectural Invariant

All stages preserve:

- `module identity` (slug/ownership/responsibility area);
- `service contract` (commands, queries, events);
- `domain rules` and policy semantics.

Only the `runtime topology` changes: embedded / remote / hybrid.

## 1.3 What Is Prohibited

- "Each crate = a microservice".
- Duplicating business logic across transport handlers.
- Direct access to foreign tables after port formalization.
- Premature migration to service-owned DB before ports/events/observability are mature.

---

## 2) Stage Structure (Mandatory Order)

1. **Stage A — Module Audit and Readiness**
2. **Stage B — Basic FBA Contracts (Before Transport)**
3. **Stage C — Event Discipline and Contract Testing**
4. **Stage D — Pilot 1 (async/read-boundary)**
5. **Stage E — Pilot 2 (Inventory Reservation)**
6. **Stage F — Pilot 3 (Payment/Fulfillment/Product read/Pricing)**
7. **Stage G — Selective Storage Decomposition and Write Extraction**

Transition to the next stage is only allowed after completing the Exit Criteria of the current one.

## 2.1 Current FBA Tracks and Unified Template

As of 2026-06-14 the repository already has several FBA tracks. They must not be migrated
in different ways: new and existing increments must converge on a single template
of `provider/consumer metadata + neutral ports + typed errors + locked contract-test/fallback-smoke metadata + fallback/rollout evidence`.

| Module | Current Role | Status | Uniform Next Step | Evidence Source |
|---|---|---|---|---|
| `page_builder` | reference provider for `preview/tree/properties/publish` | `in_progress` | Continue after first migration slice: `PageBuilderCapabilityService` already accepts `PortContext`, next step — capability handlers and contract tests until `boundary_ready` without changing provider/consumer metadata format | `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`, `crates/rustok-page-builder/docs/implementation-plan.md` |
| `pages` | first consumer of reference provider `page_builder` | `in_progress` | Replace synthetic Wave 0 evidence with actual tenant before/after snapshots and smoke/trace packet | `crates/rustok-pages/docs/implementation-plan.md`, registry page-builder |
| `commerce` | umbrella orchestration/readiness-hardening for ecommerce slices | `in_progress` | Align checkout/post-order boundaries to the same template: owner-module ports, typed errors/context, events and absence of rules in transport/UI; consumer registry now captures checkout orchestration dependencies on pricing/inventory/order/payment/fulfillment/cart provider contracts and mirrors payment/fulfillment provider SPI default-provider/lifecycle metadata, and payment/fulfillment provider SPI source markers/source paths/explicit lifecycle owner service names/default provider ids/capability fields are checked by the same fast gate | `crates/rustok-commerce/contracts/commerce-fba-registry.json`, `crates/rustok-commerce/docs/implementation-plan.md` |
| `product` | provider catalog read-projection for commerce checkout/storefront and pricing enrichment consumers | `in_progress` | Close runtime contract tests/fallback smoke for `ProductCatalogReadPort`, confirm embedded/GraphQL catalog parity snapshots before status promotion | `crates/rustok-product/src/ports.rs`, `crates/rustok-product/contracts/product-fba-registry.json`, `crates/rustok-product/docs/implementation-plan.md` |
| `ai-product` | support-consumer product catalog context for product copy / AI attributes verticals | `boundary_ready` | Next step: owner transport cutover/admin host rendering evidence; runtime source-smoke already verified by `cargo test -p rustok-ai-product --lib` | `crates/rustok-ai-product/contracts/ai-product-fba-registry.json`, `crates/rustok-ai-product/contracts/evidence/ai-product-runtime-fallback-smoke.json`, `crates/rustok-ai-product/docs/implementation-plan.md` |
| `ai-content` | support-consumer content moderation/blog draft vertical contracts | `boundary_ready` | Next step: executable host/direct runtime composition evidence; generated payload and policy fallback smoke already verified by `cargo test -p rustok-ai-content --lib` | `crates/rustok-ai-content/contracts/ai-content-fba-registry.json`, `crates/rustok-ai-content/contracts/evidence/ai-content-runtime-fallback-smoke.json`, `crates/rustok-ai-content/docs/implementation-plan.md` |
| `ai-order` | support-consumer order operator context for analytics / ops assistant verticals | `boundary_ready` | Next step: executable host/direct runtime composition evidence; order payload and live-status fallback smoke already verified by `cargo test -p rustok-ai-order --lib` | `crates/rustok-ai-order/contracts/ai-order-fba-registry.json`, `crates/rustok-ai-order/contracts/evidence/ai-order-runtime-fallback-smoke.json`, `crates/rustok-ai-order/docs/implementation-plan.md` |
| `auth` | core capability provider for identity/permission/admin auth boundary | `boundary_ready` | Next step: full admin/server transport parity evidence; permission/admin-boundary smoke already verified by `cargo test -p rustok-auth --lib` and `npm run verify:auth:admin-boundary` | `crates/rustok-auth/contracts/auth-fba-registry.json`, `crates/rustok-auth/contracts/evidence/auth-runtime-fallback-smoke.json`, `crates/rustok-auth/docs/implementation-plan.md` |
| `ai` | capability orchestrator for router/direct execution/admin transport boundaries | `in_progress` | Advance source-smoke to executable runtime fallback evidence and confirm provider fallback/direct operator-review diagnostics before `boundary_ready` | `crates/rustok-ai/contracts/ai-fba-registry.json`, `crates/rustok-ai/contracts/evidence/ai-runtime-fallback-smoke.json`, `crates/rustok-ai/docs/implementation-plan.md` |
| `pricing` | provider read-projection for checkout/product price consumers | `in_progress` | Close contract tests for `PricingReadPort` and replace embedded/GraphQL compatibility evidence with actual parity snapshots before status promotion | `crates/rustok-pricing/src/ports.rs`, `crates/rustok-pricing/contracts/pricing-fba-registry.json`, `crates/rustok-pricing/docs/implementation-plan.md` |
| `inventory` | provider availability/reservation for checkout/product inventory consumers | `in_progress` | Close contract tests/fallback smoke for `InventoryReservationPort`, confirm idempotency/write semantics and storefront projection parity before status promotion | `crates/rustok-inventory/src/ports.rs`, `crates/rustok-inventory/contracts/inventory-fba-registry.json`, `crates/rustok-inventory/docs/implementation-plan.md` |
| `order` | provider checkout completion/result for commerce orchestration | `boundary_ready` | Owner checkout-completion path has `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"` and is gated by `npm run verify:ecommerce:fba`; remote/base fallback smoke remains follow-up before `transport_verified` | `crates/rustok-order/src/ports.rs`, `crates/rustok-order/contracts/order-fba-registry.json`, `crates/rustok-order/docs/implementation-plan.md` |
| `payment` | provider payment collection create/reuse for commerce checkout | `boundary_ready` | Provider SPI live-adapter executed evidence already recorded (`payment-provider-spi-live-adapter-evidence.json`) and gated by `npm run verify:ecommerce:fba`; base `PaymentCollectionPort` runtime fallback remains follow-up before `transport_verified` | `crates/rustok-payment/src/ports.rs`, `crates/rustok-payment/contracts/payment-fba-registry.json`, `crates/rustok-payment/docs/implementation-plan.md` |
| `customer` | provider read-projection for commerce checkout/order customer consumers | `in_progress` | Close contract tests/fallback smoke for `CustomerReadPort`, confirm embedded/GraphQL checkout compatibility snapshots before status promotion | `crates/rustok-customer/src/ports.rs`, `crates/rustok-customer/contracts/customer-fba-registry.json`, `crates/rustok-customer/docs/implementation-plan.md` |
| `cart` | provider checkout lifecycle for commerce checkout consumers | `in_progress` | Close contract tests/fallback smoke for `CartCheckoutPort`, confirm embedded/GraphQL checkout/storefront compatibility snapshots and lifecycle writes before status promotion | `crates/rustok-cart/src/ports.rs`, `crates/rustok-cart/contracts/cart-fba-registry.json`, `crates/rustok-cart/docs/implementation-plan.md` |
| `tax` | provider tax calculation for cart tax-line consumers | `in_progress` | Close runtime contract tests/fallback smoke for `TaxCalculationPort`, confirm cart/order tax snapshot parity before status promotion | `crates/rustok-tax/src/ports.rs`, `crates/rustok-tax/contracts/tax-fba-registry.json`, `crates/rustok-tax/docs/implementation-plan.md` |
| `comments` | provider generic comment thread boundary for blog/commentable-surface consumers | `boundary_ready` | Execute provider runtime contract/fallback evidence and prove atomic lifecycle-event publication, Blog delivery, retry, and recovery before `transport_verified` | `crates/rustok-comments/src/ports.rs`, `crates/rustok-comments/contracts/comments-fba-registry.json`, `crates/rustok-comments/docs/implementation-plan.md` |
| `blog` | consumer generic comment thread boundary and reply-count projection from `comments` | `boundary_ready` | Execute real consumer/projection runtime delivery, duplicate-event, missing-post, retry, and recovery evidence before `transport_verified` | `crates/rustok-blog/contracts/blog-fba-registry.json`, `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`, `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`, `crates/rustok-blog/docs/implementation-plan.md` |
| `media` | provider asset read/image descriptor boundary for SEO/AI media consumers, plus owner write/control boundary | `boundary_ready` | One compiled suite verifies every read/write port operation against embedded and loopback gRPC providers, including deadlines, typed errors, owner normalization, upload-control transport, reconciliation, and delete. Isolated process/database/storage operational evidence remains required before `transport_verified` | `crates/rustok-media/src/ports.rs`, `crates/rustok-media-transport/tests/port_conformance.rs`, `crates/rustok-media/contracts/media-fba-registry.json` |
| `seo` | Media image descriptor consumer boundary | `in_progress` | Product forwards its media asset UUIDs and SEO composes `MediaAssetReadPort`; remaining target providers, tenant/degraded media execution, and public URL/proxy evidence are required before promotion | `crates/rustok-seo/contracts/seo-fba-registry.json`, `crates/rustok-seo/docs/implementation-plan.md` |
| `ai-media` | support consumer image asset descriptor boundary from `media` | `boundary_ready` | Next step: provider-side media runtime execution evidence; support-adapter fallback smoke already verified by `cargo test -p rustok-ai-media --lib` | `crates/rustok-ai-media/contracts/ai-media-fba-registry.json`, `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`, `crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json`, `crates/rustok-ai-media/docs/implementation-plan.md` |
| `ai-alloy` | support adapter script execution policy boundary for Alloy vertical | `in_progress` | Source-level policy registry captures `alloy_script_execution_policy`, `allowed_operations` and descriptor `runtime_operation`; next step — targeted Rust tests when compilations are allowed | `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json`, `crates/rustok-ai-alloy/contracts/evidence/ai-alloy-policy-static-matrix.json`, `crates/rustok-ai-alloy/docs/implementation-plan.md` |
| `search` | provider search query/suggestions boundary for storefront/admin consumers | `boundary_ready` | Executable no-compile runtime fallback/contract/invocation evidence already recorded; next step before `transport_verified` — live runtime contract execution with real provider invocation | `crates/rustok-search/src/ports.rs`, `crates/rustok-search/contracts/search-fba-registry.json`, `crates/rustok-search/docs/implementation-plan.md` |
| `fulfillment` | provider seller-aware shipping selection for commerce checkout | `boundary_ready` | Provider SPI live-adapter executed evidence already recorded (`fulfillment-provider-spi-live-adapter-evidence.json`) and gated by `npm run verify:ecommerce:fba`; base `ShippingSelectionPort` runtime fallback remains follow-up before `transport_verified` | `crates/rustok-fulfillment/src/ports.rs`, `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`, `crates/rustok-fulfillment/docs/implementation-plan.md` |
| `rbac` | provider permission-decision boundary for admin consumers | `in_progress` | Close runtime fallback smoke for `RbacPermissionDecisionPort`, confirm claims-scope/degraded action hiding semantics before status promotion | `crates/rustok-rbac/src/ports.rs`, `crates/rustok-rbac/contracts/rbac-fba-registry.json`, `crates/rustok-rbac/docs/implementation-plan.md` |
| `tenant` | provider read-projection boundary for server-host tenant resolution consumers | `boundary_ready` | No-compile fallback smoke is locked by `npm run verify:foundation:fba-runtime-smoke`; compiled runtime contract/fallback smoke for `TenantReadPort` remains `cargo test -p rustok-tenant --lib --tests`; next step before `transport_verified` is host/runtime cache parity evidence beyond source-lock | `crates/rustok-tenant/src/ports.rs`, `crates/rustok-tenant/contracts/tenant-fba-registry.json`, `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json`, `crates/rustok-tenant/docs/implementation-plan.md` |
| `workflow` | provider read-projection boundary for workflow admin consumers | `in_progress` | Close runtime contract tests/fallback smoke for `WorkflowReadPort`, confirm native/GraphQL admin read projection parity snapshots before status promotion | `crates/rustok-workflow/src/ports.rs`, `crates/rustok-workflow/contracts/workflow-fba-registry.json`, `crates/rustok-workflow/docs/implementation-plan.md` |
| `forum` | deferred consumer candidate for `page_builder` | `not_started` | Do not promote status until local consumer evidence appears; keep entry as deferred in provider registry | `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` |

Uniformity rules:

1. **FBA remains the rollout name, not a mandatory type prefix.** Code-facing contracts use neutral names (`PortContext`, `PortError`, `*Port`, `provider`, `consumer`).
2. **Status source is local `docs/implementation-plan.md`, the central board is synced in the same change.** Do not leave `not_started` if there is active FBA provider/consumer evidence.
3. **Machine-readable metadata is mandatory for provider/consumer tracks.** For `page_builder -> pages` the source is `page-builder-fba-registry.json`; subsequent tracks must reuse the same format or explicitly extend it in this plan, not create a parallel format.
4. **Neutral port primitives apply only to new/updated ports.** Existing FBA slices are not mechanically rewritten without feature work; upon the next change they are brought to the same `context/error/idempotency/deadline` requirements.
5. **Promotion to `boundary_ready` or `transport_verified` requires evidence.** Metadata or FFA split alone does not count as remote/runtime verification.

## 2.2 Structural Standard for Module Migration

### Whole-module extraction pilots: media and search

The first distributed FBA pilots are whole-module deployments, not splits of
one module into multiple microservices. `media` is the storage/read pilot and
`search` is the query/connector pilot. The modular monolith remains the default.
Media loopback transport conformance is accepted; an actual process split still
requires isolated-runtime evidence.

`rustok-search` owns the `SearchEngine` connector abstraction. PostgreSQL,
Meilisearch, Typesense, and Algolia connectors are internal adapters selected
by the search service; storefront/admin consumers call only
`SearchQueryPort` and `SearchSuggestionPort`. `rustok-index` owns canonical
document ingestion and read models. Its events feed the search service through
a replayable ingestion boundary; index deployment is not split in the first
query-service pilot.

The extraction order is:

1. contract and error-matrix hardening;
2. generic gRPC loopback conformance for the existing ports;
3. isolated `media` process/database/storage proof;
4. isolated `search` query process with internal connector selection;
5. optional index-ingestion split after replay, duplicate, lag, rebuild, and
   recovery evidence;
6. performance and operational comparison against the in-process baseline.

The authoritative decision and acceptance evidence are recorded in
`DECISIONS/2026-07-16-media-search-extraction-boundaries.md`. No other module
is promoted to a remote deployment class by this plan.

### Extraction execution status

The deployment default remains the modular monolith. Media now has its bounded
metadata/control gRPC adapter and compiled loopback conformance. This does not
authorize a production process/database split without the remaining operational
evidence.

Resume only after the platform baseline has explicit evidence for the affected
scope: reproducible targeted compilation/tests, a resolved `Cargo.lock`
baseline, owner API contracts with typed error behavior, and no known
cross-module database/service access on the paths being changed.

When the freeze is lifted, implement and verify the following order:

1. Re-audit `rustok-fba` and `rustok-runtime`; add a shared transport or
   conformance primitive only after two real module consumers demonstrate the
   same need.
2. Complete Media owner write/control coverage for upload, delete,
   translations, and reconciliation. Keep large binary data on Media-owned streaming
   REST or a presigned upload flow; do not put blobs in generic gRPC DTOs.
3. Implement the internal Search connector writer and owner ingestion/control
   contract for schema synchronization, document upsert/delete, rebuild, and
   health. Connectors remain private to `rustok-search`.
4. Remove Search query-time SQL access to `index_product_categories` and
   `index_product_attribute_values`. Populate the needed category and facet
   fields in Search-owned projections during ingestion; use
   `IndexReadModelPort` only for optional enrichment.
5. Media loopback gRPC conformance is complete. Add isolated database/storage
   evidence for Media, then add the isolated Search query-service profile.
6. Promote no readiness status without compiled and live evidence for tenant,
   authorization, retry/restart, health/metrics, degraded behavior, and
   rollback/recovery.

Yes, there is a single standard. For each new FBA increment the same
artifact structure is mandatory; absence of any item below is considered a gap and does not allow
status above `in_progress`:

1. **Local source of truth:** `crates/<module>/docs/implementation-plan.md` contains
   `## FFA/FBA status`, current role (`provider`, `consumer`, `orchestrator`, `support`)
   and evidence on boundary/metadata/verification.
2. **Central status:** `docs/modules/registry.md` contains a synchronized readiness board row
   with the same FBA status and a link to the local plan.
3. **Runtime metadata:** `rustok-module.toml` or module-owned machine-readable registry
   captures the provider/consumer dependency profile, contract versions, degraded modes and
   toggle/fallback profiles if the module participates in a provider/consumer track.
4. **Contract location:** transport-neutral DTO/port/error contracts live in the owner module
   or a shared foundation crate only if they are truly cross-module; host apps do not
   become owners of domain/application contracts.
5. **Verification location:** alongside machine-readable metadata there is an anti-drift/fallback gate
   (`scripts/verify/*` or module-local verifier), and the local plan lists command/evidence.
6. **Evidence packet:** for Wave/pilot rollout there are actual or explicitly marked
   synthetic before/after snapshots, smoke outcomes, metrics/traces and keep/rollback decision.
7. **Docs sync:** if FBA status, provider/consumer metadata, ports/events, routing,
   tenancy, UI contract or observability changes, local docs, central board
   and this unified plan are updated simultaneously if the standard itself changes.


As of 2026-06-16 the ecommerce provider track additionally received a unified static evidence layer for future contract tests: `pricing`, `inventory`, `order`, `payment`, `fulfillment`, `customer` and `cart` have `contracts/evidence/*-contract-test-static-matrix.json`, the command `npm run verify:ecommerce:fba` runs registry + evidence gates, including checking evidence packet compliance with provider registry cases/fallback profiles via `npm run verify:ecommerce:fba-contract-evidence`. This still does not promote status to `boundary_ready`: runtime execution and fallback smoke remain separate gates.

As of 2026-06-29 `payment` and `fulfillment` have been promoted to `boundary_ready` based on provider SPI live-adapter executed evidence: `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json` and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json` capture concrete external adapter contract execution for guarded invocation, typed provider-error mapping, degraded fallback, unavailable-mode blocking and webhook/tracking replay delegation. Base port contract/fallback smoke for `PaymentCollectionPort` and `ShippingSelectionPort` remains follow-up before `transport_verified`.

As of 2026-06-18 the fast gate `npm run verify:ecommerce:fba-registries` additionally checks in-process provider implementations at each operation level, if the registry declares `in_process_provider_impl`: read operations must call `require_deadline_semantics()?`, write operations with `idempotency_required = true` must call `require_write_semantics()?`, and read operations must not accidentally require write-idempotency. This closes anti-drift for typed context/deadline/idempotency semantics without running expensive compilation, but also does not promote status without runtime contract execution. For `order.checkout_completion.v1` the same fast gate now additionally blocks premature `OrderService` in-process implementation (both registry metadata and source-level impl) until the registry contains `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`; this protects the checkout completion boundary from a fake embedded provider without cart/result projection evidence.

As of 2026-06-29 `order` is promoted to `boundary_ready`: `crates/rustok-order/contracts/order-fba-registry.json` contains `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`, and `npm run verify:ecommerce:fba` checks the owner `OrderService` implementation for write/read policy, lifecycle delegation, locale-aware snapshot reload and typed unavailable result projection. Remote/base fallback smoke remains a condition before `transport_verified`.

As of 2026-06-18 `product` added to the ecommerce provider track as a catalog read provider: `ProductCatalogReadPort`/`product.catalog_read.v1`, registry `crates/rustok-product/contracts/product-fba-registry.json` and static evidence `crates/rustok-product/contracts/evidence/product-contract-test-static-matrix.json` are included in `npm run verify:ecommerce:fba` together with product dependency in the commerce consumer registry.


As of 2026-06-18 `customer` added to the ecommerce provider track as a customer read-projection provider: `CustomerReadPort`/`customer.read_projection.v1`, registry `crates/rustok-customer/contracts/customer-fba-registry.json` and static evidence `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json` are included in `npm run verify:ecommerce:fba` together with customer dependency in the commerce consumer registry.


`cart` is an ecommerce provider through `CartCheckoutPort`/`cart.checkout.v2`, covering checkout snapshots and lifecycle writes. Its registry and static evidence are included in `npm run verify:ecommerce:fba` together with the cart dependency in the commerce consumer registry.

As of 2026-06-18 `tax` added as a support provider track for cart tax calculation: `TaxCalculationPort`/`tax.calculation.v1`, registry `crates/rustok-tax/contracts/tax-fba-registry.json` and static evidence `crates/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:tax:fba` without promotion to `boundary_ready` until runtime execution/fallback smoke.

As of 2026-06-30 the batch commerce-domain wave added no-compile runtime-order evidence for `product`, `pricing`, `inventory`, `customer`, `cart` and `tax`: module-owned `contracts/evidence/*-runtime-contract-smoke.json` are checked by the shared `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` with fixture regressions. The gate fixes the order of shared policy/idempotency -> owner provider -> typed error mapping and parity fallback/degraded metadata, but intentionally does not promote modules above `in_progress` until live provider execution.

As of 2026-06-30 the owner-provider wave added a unified no-compile runtime-order gate for `comments`, `rbac`, `workflow`, `region`, `media` and `outbox`: `contracts/evidence/*-provider-runtime-order-smoke.json` and `scripts/verify/verify-owner-fba-runtime-order.mjs` capture shared policy, idempotency, tenant/request validation, owner invocation/evaluation and typed error mapping. `comments` uses canonical `PortCallPolicy::write()`, which already applies write semantics inside the shared primitive. As of 2026-07-01 `outbox` registry/manifest migrated to the single `rustok_api::ports` contract, and the outbox-specific adapter moved to the owner crate, eliminating the reverse runtime dependency from `rustok-api`. Statuses are not promoted without live execution.

As of 2026-06-30 the orchestrator/capability wave added a unified no-compile runtime-order gate for `ai` and `page_builder`: `contracts/evidence/*-orchestrator-runtime-order-smoke.json` and `scripts/verify/verify-orchestrator-fba-runtime-order.mjs` capture AI support-adapter registry parity, direct runtime registration APIs, router fallback diagnostics markers, native/GraphQL admin transport boundary, and page-builder capability flag -> `PortCallPolicy` -> owner service call and authorization -> service call order. Statuses remain `in_progress` until live runtime/tenant evidence.

The consumer wave uses a unified no-compile runtime-order gate for `blog` and `seo`. SEO composes `MediaAssetReadPort` through `SeoMediaAssetReadProvider`; Product supplies its media UUID, while URL-only target records keep their owner descriptor. The static gate does not substitute for tenant/degraded consumer runtime execution, so statuses remain `in_progress`.

As of 2026-06-18 `comments` added as a provider track for generic comment threads: `CommentsThreadPort`/`comments.thread.v1`, registry `crates/rustok-comments/contracts/comments-fba-registry.json` and static evidence `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:comments:fba` without promotion to `boundary_ready` until runtime execution/fallback smoke.

As of 2026-06-19 `search` added as a provider track for search query/suggestions boundary: `SearchQueryPort`/`SearchSuggestionPort`/`search.query.v1`, registry `crates/rustok-search/contracts/search-fba-registry.json` and static evidence `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:search:fba`; as of 2026-06-29 executable no-compile runtime fallback/contract/invocation evidence promoted FBA status to `boundary_ready`, and live runtime contract execution remains a condition for `transport_verified`.

As of 2026-06-19 `blog` added as a consumer track for `comments.thread.v1`: registry `crates/rustok-blog/contracts/blog-fba-registry.json`, static evidence `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`, no-compile source-smoke `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`, manifest `[fba.consumer]` and fast gate `npm run verify:blog:fba` capture `CommentsThreadPort` dependency, fallback profiles and degraded modes without promotion to `boundary_ready` until real runtime execution.

As of 2026-07-22 `media` has compiled embedded/loopback runtime conformance for every `MediaAssetReadPort` and `MediaAssetWritePort` operation in `crates/rustok-media-transport/tests/port_conformance.rs`. The fast `npm run verify:media:fba` gate locks this runtime evidence to the provider registry and static matrix; FBA remains `boundary_ready` until isolated deployment evidence supports `transport_verified`.

As of 2026-06-19 `seo` added as a consumer track for `media.asset_read.v1`: registry `crates/rustok-seo/contracts/seo-fba-registry.json`, static evidence `crates/rustok-seo/contracts/evidence/seo-media-consumer-static-matrix.json`, manifest `[fba.consumer]` and fast gate `npm run verify:seo:fba` capture `MediaAssetReadPort` dependency, provider fallback-smoke source, fallback profiles and degraded modes in `source_locked_pending_consumer_runtime` state without promotion to `boundary_ready` until consumer runtime execution/fallback smoke.

As of 2026-06-20 `ai-media` has a support-consumer track for `media.asset_read.v1`: registry `crates/rustok-ai-media/contracts/ai-media-fba-registry.json`, static evidence `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`, runtime fallback smoke `crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json` and gate `npm run verify:ai-media:fba` capture `MediaAssetReadPort` dependency, adapter source markers, fallback profile and degraded mode; as of 2026-06-29 support-adapter smoke confirmed by `cargo test -p rustok-ai-media --lib` and status promoted to `boundary_ready`. `ai-alloy` added as a source-level policy track: `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json` and `alloy_script_execution_policy` capture ownership of script runtime payload policy, allowed operations and descriptor runtime operation while runtime composition stays in `rustok-ai`.

As of 2026-06-19 `workflow` added as a provider track for admin read-projection boundary: `WorkflowReadPort`/`workflow.read_projection.v1`, registry `crates/rustok-workflow/contracts/workflow-fba-registry.json` and static evidence `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:workflow:fba` without promotion to `boundary_ready` until runtime execution/fallback smoke.

As of 2026-06-19 `rbac` added as a provider track for admin permission-decision boundary: `RbacPermissionDecisionPort`/`rbac.permission_decision.v1`, registry `crates/rustok-rbac/contracts/rbac-fba-registry.json` and static evidence `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:rbac:fba` without promotion to `boundary_ready` until runtime fallback smoke.

As of 2026-06-20 `tenant` added as a provider track for tenant read-projection boundary: `TenantReadPort`/`tenant.read_projection.v1`, registry `crates/rustok-tenant/contracts/tenant-fba-registry.json` and evidence `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json` are checked by gate `npm run verify:tenant:fba`; as of 2026-06-29 runtime contract/fallback smoke executed via `cargo test -p rustok-tenant --lib --tests`, evidence marked `runtime_verified`, and status promoted to `boundary_ready`; the current no-compile package `scripts/verify/verify-foundation-fba-runtime-smoke.mjs` adds source-locked fallback smoke `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json` without running compilation.

As of 2026-06-20 `region` added as a provider track for region/country read-projection boundary: `RegionReadPort`/`region.read_projection.v1`, registry `crates/rustok-region/contracts/region-fba-registry.json` and static evidence `crates/rustok-region/contracts/evidence/region-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:region:fba` without promotion to `boundary_ready` until runtime contract/fallback smoke.

As of 2026-06-20 `channel` added as a provider track for channel/default/host-target read-projection boundary: `ChannelReadPort`/`channel.read_projection.v1`, registry `crates/rustok-channel/contracts/channel-fba-registry.json` and static evidence `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:channel:fba`; as of 2026-06-29 no-compile executable fallback smoke promoted FBA status to `boundary_ready`, and full Rust runtime contract/fallback evidence remains a condition for `transport_verified`.

As of 2026-06-20 `index` added as a provider track for indexed read-model/rebuild boundary: `IndexReadModelPort`/`index.read_model.v1` and `IndexRebuildPort`/`index.rebuild.v1`, registry `crates/rustok-index/contracts/index-fba-registry.json` and static evidence `crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:index:fba`; as of 2026-06-29 no-compile source-locked runtime fallback smoke promoted FBA status to `boundary_ready`, and persistence-backed Rust runtime contract/fallback evidence remains a condition for `transport_verified`.

As of 2026-06-20 `outbox` added as a provider track for relay worker control boundary: `OutboxRelayPort`/`outbox.relay_control.v1`, registry `crates/rustok-outbox/contracts/outbox-fba-registry.json` and static evidence `crates/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:outbox:fba` without promotion to `boundary_ready` until runtime contract/fallback smoke.

As of 2026-06-20 `email` added as a provider track for transactional delivery boundary: `EmailDeliveryPort`/`email.delivery.v1`, registry `crates/rustok-email/contracts/email-fba-registry.json` and evidence `crates/rustok-email/contracts/evidence/email-contract-test-static-matrix.json` are checked by gate `npm run verify:email:fba`; as of 2026-06-29 targeted delivery-port tests executed via `cargo test -p rustok-email --lib`, disabled-provider noop fallback marked `runtime_verified`, and status promoted to `boundary_ready`; the current no-compile package `scripts/verify/verify-foundation-fba-runtime-smoke.mjs` adds source-locked fallback smoke `crates/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json` for disabled noop/SMTP delivery profiles without running compilation.

The structure check on the current state revealed one fixed gap: `page_builder` already
had FBA provider metadata and registry but was absent from the readiness board and had no local
FFA/FBA status block. Now `page_builder` and `pages` are reflected uniformly: local plan +
central board + machine-readable registry/evidence. Remaining gaps are not a violation of the
standard because they are explicitly recorded as `not_started`/`deferred` or as compile/runtime
blockers in verification output.

## 2.3 Alignment with Target Crate Structure

The previously proposed schema is close to the target model, but in RusTok it is applied with adjustments
based on the current modular platform:

```text
crates/rustok-<module>/
  src/dto|domain|error      # domain types, DTO, errors; folder names may differ
  src/services|ports        # service layer and/or explicit owner-module ports
  src/entities|migrations   # SeaORM storage ownership; repository interfaces appear when remote/test seam is needed
  src/graphql|controllers   # transport adapters, thin mapping on top of service/port
  admin|storefront          # optional module-owned UI packages with core/transport/ui split
  rustok-module.toml        # runtime metadata, dependencies, provider/consumer FBA sections
  contracts/                # optional machine-readable registry/evidence for provider/consumer tracks

crates/rustok-<module>-grpc/ # optional late-stage adapter crate, not a default requirement
  proto/schema              # gRPC/protobuf contract only after ADR/DoR
  server adapter            # calls the same service/port, does not contain domain rules
  client adapter            # remote implementation of the same port
  PortContext/error mapping # mapping neutral port primitives to transport metadata/status

apps/server/
  composition/root wiring   # module registry, GraphQL/REST/controllers, health/metrics
  transport selection       # future per-module runtime profile; currently mostly in-process/native/GraphQL
  public API                # host API does not own domain rules
```

Key differences from the `service trait + in-process impl + repository interfaces` schema as a
mandatory template:

1. **Trait-port is not introduced mechanically.** If a module is not yet ready for remote/profile split,
   the service struct remains a valid owner service layer; trait/adapter is extracted on the first
   real boundary or contract-test increment.
2. **Repository interfaces are not mandatory from the first PR.** Currently many modules own SeaORM
   entities/migrations directly; the abstraction seam is added when a remote adapter,
   test double or foreign-table access prohibition is needed.
3. **`rustok-<module>-grpc` is a late optional adapter.** Until DoR is closed, a gRPC
   crate should not be created just for form's sake; first you need stable port, `PortContext`, typed errors, events/outbox,
   contract tests and ADR.
4. **Transport adapters already exist, but not all are remote.** GraphQL/REST/`#[server]` live as
   thin adapters on top of owner service/port; gRPC will be another adapter profile, not a new
   business logic implementation.
5. **Machine-readable provider/consumer metadata is already part of the structure.** For
   `page_builder -> pages` this is `page-builder-fba-registry.json` + `rustok-module.toml`; new
   provider/consumer tracks must repeat this pattern or extend it within the unified plan.

In summary: the concept matches by layers (`domain/service-port/implementation/storage/adapter/server
wiring`), but the RusTok standard does not require creating all folders and `*-grpc` crate ahead of time. The structure
evolves through readiness gates to avoid getting formal interfaces without evidence.

---

## 3) Stage A — Audit and Readiness Matrix

## 3.1 Mandatory Artifacts

- `Module Inventory Table` (for each target module):
  - slug, owner, owned storage, public use-cases;
  - incoming/outgoing events;
  - dependencies (Cargo + modules graph);
  - role: orchestrator/facade, write-model owner, read-model provider, support service.
- `Coupling Debt Register`:
  - direct calls to neighboring domains;
  - direct SQL to foreign tables;
  - missing idempotency/deadline;
  - event gaps (no outbox/versioning/replay policy).
- `Readiness Matrix`: High / Medium / Low.

## 3.2 Stage A Exit Criteria

- All modules in the target scope have a filled inventory row.
- For each Medium/Low module a remediation backlog is recorded.
- For each remote candidate there is an ADR draft with risks and rollback approach.

---

## 4) Stage B — Basic FBA Contracts (Ports before transports)

## 4.1 Unified `PortContext`

The initial shared implementation lives in `rustok-api::ports` and intentionally remains transport-agnostic: it is a contract primitive for ports/adapters, not a domain service.

Mandatory fields:

- tenant;
- actor/service identity;
- claims/role;
- channel + locale;
- correlation/causation + trace context;
- idempotency key (write);
- deadline/timeout/cancellation.

Rule: passed as an explicit parameter of each port.

## 4.2 Unified Error Model

A single set of domain errors (validation/not_found/conflict/forbidden/unavailable/timeout/invariant violation) + predictable mapping to REST/GraphQL/gRPC.

## 4.3 Port Layer

Minimum target set of ports:

- `ProductPort`, `PricingPort`, `InventoryPort`, `CartPort`,
- `OrderPort`, `PaymentPort`, `FulfillmentPort`, `TaxPort`.

Requirement: in-process impl first, then remote adapters.

## 4.4 Data Ownership Policy

- A module reads/writes only its own storage.
- Cross-module data access — only via port/snapshot DTO/read model.

## 4.5 Stage B Exit Criteria

- All target ports defined in transport-agnostic form.
- `PortContext` and error model used in all new/updated port calls.
- New direct foreign-table accesses are not allowed.

---

## 5) Stage C — Events, Outbox and Contract Testing

## 5.1 Event Vocabulary

For critical domains define a versioned vocabulary (e.g.: `ProductPublished`, `PriceChanged`, `InventoryReserved`, `OrderPlaced`, `PaymentAuthorized`).

Each event must have: tenant, aggregate id, schema version, correlation/causation, idempotency semantics.

## 5.2 Outbox Discipline

- Write domain state + outbox in one transaction.
- Publish via worker/dispatcher.
- Consumers are idempotent + replay-safe + tolerant to out-of-order.

## 5.3 Contract Tests

For each port the same test suite runs:

- against in-process impl;
- against remote adapter.

Business result must match; differences allowed only in latency/failure envelope.

## 5.4 Stage C Exit Criteria

- Outbox is enabled for all write owners in the pilot scope.
- Contract tests exist for all ports in the pilot scope.
- There are replay/idempotency/out-of-order scenarios in tests.

---

## 6) Stage D — Pilot 1 (async/read-boundary)

## 6.1 Candidates

- search/indexing;
- AI enrichment/recommendations.

## 6.2 Steps

1. Extract boundary into port and adapter (gRPC or async worker — depending on use-case nature).
2. Wire up embedded/remote switching via runtime config.
3. Move host/facade calls to the port.
4. Verify SLO: latency, error rate, throughput, retry behavior.

## 6.3 Exit Criteria

- Functional parity with embedded profile confirmed.
- Metrics and tracing stable for at least the agreed observation window.

---

## 7) Stage E — Pilot 2 (Inventory Reservation)

## 7.1 Steps

1. Introduce `reservation` model: idempotency key, TTL/expiration, status lifecycle.
2. Establish events: `InventoryReserved`, `InventoryReservationReleased`, `InventoryAdjusted`.
3. Implement `InventoryPort` remote server/client.
4. Embed compensations in the checkout saga (`release_reservation`).
5. Run load tests on peak checkout scenarios.

## 7.2 Exit Criteria

- Reservation commands are retry-safe.
- Compensations correctly handle controlled failures.
- Load profile does not degrade below agreed thresholds.

---

## 8) Stage F — Pilot 3 (Payment/Fulfillment/Product read/Pricing)

Order is mandatory:

1. `PaymentPort` and `FulfillmentPort` as remote adapters (external providers).
2. `ProductPort` read-side snapshots (`get_product_snapshot`, `list_publishable_catalog_page`).
3. `PricingPort` after product read contracts are stabilized.
4. `TaxPort` as an explicit support boundary (embedded/stateless remote/provider adapter — decided by ADR).

## 8.1 Exit Criteria

- No direct reading of product internals from pricing.
- Checkout orchestration works through ports with the same business results.
- Synchronous path and async post-processing are separated architecturally.

---

## 9) Stage G — Late Stages (Storage and Write Extraction)

Allowed storage modes:

1. shared DB + in-process;
2. shared DB + remote process;
3. service-owned DB;
4. read-model replica/projection.

Rule: transition to `service-owned DB` only after stable remote operation of the module, mature saga/outbox model and approved ADR.

---

## 10) Unified Definition of Ready for Module Remote Migration

A module can be moved to a remote profile only when **all** conditions are met:

1. Stable transport-agnostic port + contract tests (in-process/remote).
2. Full `PortContext` on all commands/queries.
3. Outbox + versioned events + replay/idempotency policy.
4. No foreign-table access outside owner boundary.
5. Write methods have idempotency key and deadline semantics.
6. Health/readiness/metrics/tracing parity between profiles.
7. Separate ADR with reasons, risks, rollback and ownership impact.

---

## 11) Minimum Quarterly Rollout (Template)

- **Q1:** Stages A+B.
- **Q2:** Stage C + Pilot 1.
- **Q3:** Pilot 2.
- **Q4:** Pilot 3 + decisions on selective storage evolution.

If stage Exit Criteria are not met, the next quarterly step does not start.

---

## 12) Document Change Management

- This document is the canonical FBA implementation plan.
- Changes to sequence/criteria are made only together with updating related ADRs.
- New "parallel FBA plans" are not created; extensions are added here.
