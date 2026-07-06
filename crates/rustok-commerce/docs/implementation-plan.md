# Implementation plan for `rustok-commerce`

## Execution checkpoint

- Current phase: ecommerce FFA checkout handoff hardening
- Last checkpoint: Storefront checkout runtime no longer accepts Loco `AppContext`: `src/storefront_checkout_runtime.rs` exposes `StorefrontCheckoutRuntime` with explicit DB/event bus handles, and payment/order/fulfillment owner storefront SSR adapters compose this runtime at their host boundary. Product/catalog, storefront order/cart/checkout, admin order/change/return, fulfillment, shipping and payment HTTP controller slices also migrated to `CommerceHttpRuntime`: `src/controllers/products.rs`, `src/controllers/admin/products.rs`, `src/controllers/store/products.rs`, `src/controllers/store/orders.rs`, `src/controllers/store/carts.rs`, `src/controllers/store/checkout.rs`, `src/controllers/admin/orders.rs`, `src/controllers/admin/changes.rs`, `src/controllers/admin/returns.rs`, `src/controllers/admin/fulfillments.rs`, `src/controllers/admin/shipping.rs` and `src/controllers/admin/payments.rs` no longer accept Loco `AppContext`; product/catalog/order/cart/checkout/change/return files also do not use `rustok_outbox::loco`; remaining admin/storefront REST adapters remain as the next Loco-exit slices. Batch payment read cutover is complete: `rustok-payment-storefront` owns collection and refund-summary DTO/request contracts, MissingServer-gated native/GraphQL transports and endpoints `payment/payment-collection` / `payment/refund-summary`; commerce aggregate no longer contains raw payment/refund GraphQL, payment DTO mapping or decimal aggregation.
- Next step: Reduce aggregate cart projection only as a whole owner-handoff package; production provider adapter wiring should be done separately from the storefront boundary.
- Open blockers: None.
- Hand-off notes for next agent: After each post-order operator UI/page addition, update this checkpoint block and central registry evidence; keep the Next host route as a thin auth/options adapter only.
- Last updated at (UTC): 2026-06-30T14:34:52Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress` (readiness hardening for already ready slices; remote transport/runtime profile not yet considered included)
- FBA contract version: `commerce.checkout_orchestration.fba.v1`
- Product catalog read provider dependency is now tracked through `crates/rustok-product/contracts/product-fba-registry.json` / `ProductCatalogReadPort` with static contract-test evidence; commerce remains the orchestrator/consumer and does not own product catalog rules.
- Customer read-projection provider dependency is now tracked through `crates/rustok-customer/contracts/customer-fba-registry.json` / `CustomerReadPort` with static contract-test evidence; cart checkout snapshot provider dependency is tracked through `crates/rustok-cart/contracts/cart-fba-registry.json` / `CartSnapshotReadPort`; commerce remains the orchestrator/consumer and does not own customer profile or cart lifecycle rules.
- Structural shape: `core_transport_ui`
- Evidence:
  - verification as of 2026-06-30: full `npm run verify:ffa:ui:migration`, `npm run verify:ecommerce:fba`, payment/commerce storefront unit tests and GraphQL surface regression pass; SSR/all-features `cargo check` passes for commerce/payment storefront boundary;
  - payment collection storefront read parity as of 2026-06-30: GraphQL `storefrontPaymentCollection(cartId)` and native `payment/payment-collection` check tenant/cart customer ownership before owner-service read; commerce aggregate calls `rustok_payment_storefront::transport::fetch_payment_collection`, and direct `rustok_payment::PaymentService` and local payment DTO mapper removed from `rustok-commerce-storefront`;
  - payment refund-summary handoff as of 2026-06-30: payment storefront owns `RefundSummaryFetchRequest` / `RefundSummary`, GraphQL `storefrontRefunds` projection, native `payment/refund-summary` and MissingServer-only fallback; access-checked runtime first confirms tenant/customer ownership of the order, and commerce storefront no longer contains refund query/types/summarizer and dependency on `rust_decimal`;
  - compiled evidence as of 2026-06-30: `cargo check --workspace` passed; `cargo test -p rustok-commerce -p rustok-email --no-run --locked` compiled all unit/integration test targets after moving test-only imports to explicit owner/module paths and relocating GraphQL surface regression from removed `admin/src/api.rs` / `storefront/src/api.rs` to canonical commerce and owner transport adapters;
  - module plan synchronized with central FFA/FBA readiness board; UI surface already published and managed in migration/backlog rhythm;
  - `StoreContextService` no longer owns a concrete `RegionService`: the single constructor accepts runtime-composed `Arc<dyn RegionReadPort>`, and region/country resolution goes through `PortContext` and `RegionReadRequest`; old concrete constructor/path removed, all commerce/server/storefront call sites migrated atomically;
  - `CheckoutService` accepts runtime-composed `Arc<dyn InventoryReservationPort>` for constructor compatibility with runtime composition, but public-channel availability validation is moved to inventory-owned facade `check_variant_availability_for_public_channel`; backorder policy and channel-visible stock lookup are no longer duplicated in commerce orchestration;
  - REST/GraphQL cart line-item resolution and quantity update also pass variant/channel/quantity to `check_variant_availability_for_public_channel`; generic `InventoryReservationPort::check_availability` is not used for public-channel availability, which is locked by `verify-inventory-admin-boundary.mjs`;
  - storefront REST/GraphQL region lists call `RegionReadPort::list_regions_for_tenant`; umbrella re-exports `rustok_commerce::RegionService` and `rustok_commerce::services::region` removed, so the owner service is no longer masked by the commerce facade;
  - umbrella re-export `rustok_commerce::{services::inventory, InventoryService}` removed; server migration smoke and commerce integration callers import owner service only from `rustok-inventory`;
  - umbrella re-exports `rustok_commerce::{services::customer, CustomerService}` removed; commerce transports and integration callers import owner service only from `rustok-customer`;
  - umbrella re-exports `rustok_commerce::{services::pricing, PricingService}` and pricing DTO aliases under `rustok_commerce::services::*` are removed; commerce call sites import owner service and pricing DTOs directly from `rustok-pricing`;
  - umbrella re-exports `rustok_commerce::{services::catalog, CatalogService}` are removed; commerce, server migration smoke and AI callers import the product owner service directly from `rustok-product`;
  - public owner DTO re-exports under `rustok_commerce::dto::*` are removed for product/cart/payment/order/fulfillment/customer/region; external callers import DTO contracts from owner crates (`rustok-product::dto`, `rustok-cart::dto`, `rustok-payment::dto`, `rustok-order::dto`, `rustok-fulfillment::dto`, `rustok-customer::dto`, `rustok-region::dto`), while `rustok-commerce::dto` only exposes commerce-owned checkout/context/shipping-profile DTOs;
  - public owner entity re-exports under `rustok_commerce::entities::*` are removed for product/pricing/inventory/region; commerce keeps only shipping-profile entities, while callers import product entities from `rustok-product::entities`, pricing entities from `rustok-pricing::entities`, inventory entities from `rustok-inventory::entities` and region entities from `rustok-region::entities`;
  - root GraphQL and state-machine aliases under `rustok_commerce::{CommerceQuery, CommerceMutation, Order, OrderError, Pending, Confirmed, Paid, Shipped, Delivered, Cancelled}` are removed; callers use explicit module paths `rustok_commerce::graphql::*` and `rustok_commerce::state_machine::*`;
  - umbrella re-exports `rustok_commerce::{services::cart, CartService}` and `rustok_commerce::{services::payment, PaymentService}` are removed; commerce REST/GraphQL/storefront/test call sites import owner services directly from `rustok-cart` and `rustok-payment`;
  - umbrella re-exports `rustok_commerce::{services::order, OrderService}` and `rustok_commerce::{services::fulfillment, FulfillmentService}` are removed; commerce REST/GraphQL/admin/storefront/test call sites import owner services directly from `rustok-order` and `rustok-fulfillment`;
  - admin return decision tree now has transport parity (`/admin/orders/{id}/returns/decision` ↔ `createOrderReturnDecision`) over a unified `PostOrderOrchestrationService`, including completion semantics for `return_only/refund/exchange/claim`, without duplicating rules in host/UI adapters; live REST and GraphQL parity tests lock claim → completed return + `order_change(change_type=claim)`;
  - module-owned admin UI received native-first post-order change operator: operators filter order changes by `order_id/status` and call `OrderService::apply_order_change` / `cancel_order_change` through module-owned `#[server]` functions with targeted SSR coverage, while GraphQL `orderChanges` / `applyOrderChange` / `cancelOrderChange` are preserved as fallback transport when the native server-function transport is unavailable;
  - Next Admin commerce package already publishes module-owned pages for shipping profiles, cart promotions, return decisions and order changes; host route group `/dashboard/commerce/*` is now closed by shared `ModuleGuard(slug=commerce)`, so the shell does not open operator surfaces for a disabled module and remains a thin auth/options adapter;
  - exchange/claim return-decision helper metadata now marks created order changes with `return_decision_action` and `return_decision_source`, and the admin operator workspace shows resolution summary cards from preview/metadata through framework-agnostic `admin/src/core/` helper without moving domain rules to host or Leptos render adapter;
  - FFA admin transport module split: `admin/src/lib.rs` no longer contains Leptos render/business code and only wires modules + re-exports `CommerceAdmin`; `admin/src/core/mod.rs` re-exports subdomain files for form/command/view-model policy, and `admin/src/transport/mod.rs` re-exports shipping-profile, cart-promotion and order-change transport operations over the existing native/GraphQL-capable `api` layer;
  - FFA admin/storefront transport/core split: aggregate checkout route now builds `FetchCommerceRequest`, `CartCommandRequest`, `SelectShippingOptionRequest` and commerce-owned context fallback view-model (`tenant/channel/resolution`) in Leptos-free `storefront/src/core/` submodules (`requests`, `presentation`); cart totals/line-items/adjustments stay in `rustok-cart`, payment details stay in `rustok-payment`, order totals stay in `rustok-order`, fulfillment/shipping option details stay in `rustok-fulfillment`; payment/order/fulfillment storefront packages own their result DTOs, GraphQL mutation payloads/mappers, native `#[server]` endpoint shells and MissingServer-gated fallback facades, while commerce exposes `src/storefront_checkout_runtime.rs` as the explicit checkout orchestration API over `StorefrontCheckoutRuntime` with explicit DB/event bus handles and no Loco `AppContext`; checkout owner-fragment label/data mappers live in Leptos-free `storefront/src/core/presentation.rs`; legacy `storefront/src/api.rs` removed, `storefront/src/transport/raw_adapter.rs` keeps only the aggregate read contract/native endpoint `commerce/storefront-data`, and `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` forbids owner mutations/mappers/endpoints from returning to commerce; legacy `admin/src/api.rs` also removed, admin raw native/GraphQL operations temporarily live in `admin/src/transport/raw_adapter.rs`, and `scripts/verify/verify-commerce-admin-boundary.mjs` forbids return of legacy admin API;
  - further status promotion is performed only together with verification evidence and updating local+central docs;
  - FBA-readiness gate is included for already ready ecommerce slices before expanding the roadmap with new marketplace/provider modules: checks service-contract ownership, typed request context/errors, explicit cross-module ports/events and absence of business logic in transport/UI adapters.
  - consumer-side FBA metadata is now locked in `crates/rustok-commerce/contracts/commerce-fba-registry.json`: commerce explicitly lists provider contracts for pricing/inventory/order/payment/fulfillment/product/customer/cart, checkout profiles, degraded modes and fallback profiles, `src/fba.rs` publishes typed embedded registry entrypoint for runtime/composition code, the aggregate verifier cross-checks these entries with owner provider registries, and the batch invocation trace `crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json` source-locks product/pricing/inventory/customer/cart/tax provider smoke packets against consumer fallback/degraded rows without promoting FBA beyond `in_progress`;
  - checkout orchestration now uses runtime-composed `PaymentProviderRegistry` / `FulfillmentProviderRegistry` and calls owner `execute_authorize`, `execute_capture` and `execute_create_label` seams before `PaymentService`/`FulfillmentService` lifecycle persistence; post-checkout admin REST/GraphQL payment cancel/refund creation now goes through `PaymentOrchestrationService`, which calls owner `execute_cancel` / `execute_refund` seams before `PaymentService` lifecycle persistence; provider SPI live-adapter executed evidence is now locked for payment/fulfillment in `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json` and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`; verifier requires pass-cases for guarded invocation, typed error mapping, degraded fallback, unavailable blocking and webhook/tracking replay delegation without running Cargo compilation, and `src/providers.rs` now contains source-locked owner registry `execute_*` seams for guarded async invocation before production adapter wiring;
  - provider registry evidence is locked for `crates/rustok-pricing/contracts/pricing-fba-registry.json`, `crates/rustok-inventory/contracts/inventory-fba-registry.json`, `crates/rustok-order/contracts/order-fba-registry.json`, `crates/rustok-payment/contracts/payment-fba-registry.json`, `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`, `crates/rustok-product/contracts/product-fba-registry.json`, `crates/rustok-customer/contracts/customer-fba-registry.json` and `crates/rustok-cart/contracts/cart-fba-registry.json`, so the commerce local plan does not diverge from the consumer registry;
  - Phase 11 provider SPI baseline started without vendor-specific adapters: payment-owned `src/providers.rs` locks manual provider capabilities and adapter trait for authorize/capture/cancel/refund, fulfillment-owned `src/providers.rs` locks manual carrier capabilities and adapter trait for quote/label/cancel, and lifecycle persistence remains in `PaymentService` / `FulfillmentService`;
  - provider SPI static + runtime-smoke evidence now locks payment/fulfillment operation cases, typed webhook adapter operations, owner-side external adapter registration source contracts, owner provider registry composition seams, registration failure cases, side-effect-free runtime-mode guardrails and live external gateway/carrier execution-plan requirements and dedicated live-adapter contract packets in `crates/rustok-payment/contracts/evidence/payment-provider-spi-static-matrix.json`, `crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json`, `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-contract.json`, `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`, `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json` and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-contract.json`; aggregate `npm run verify:ecommerce:fba` runs `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` together with registry/port evidence gates, but FBA status remains `in_progress` until runtime execution;
- Last verified at (UTC): 2026-06-30T14:34:52Z
- Owner: `rustok-commerce` module team

## Document status

This document captures the current roadmap of the umbrella module `rustok-commerce` after abandoning the legacy REST surface `/api/commerce/*` and after the appearance of the platform-level `rustok-channel`.

This roadmap update was performed on April 8, 2026: the UI split of the ecommerce family
has moved from a purely planned state to an active execution phase, because `product`
already received its own module-owned admin route, shipping options moved to
`fulfillment`, order operations moved to `order`, inventory visibility and targeted stock/reservation/availability actions moved to
`inventory`, pricing visibility moved to `pricing`, customer operations moved
to `customer`, region CRUD moved to `region`, and the aggregate `commerce` UI is cleaned up to
typed shipping-profile registry plus aggregate cart-promotion operator surface.

Original assumptions:

- the live REST contract for ecommerce lives at `/store/*` and `/admin/*`;
- GraphQL remains a supported transport layer;
- `rustok-commerce` continues to play the role of the root umbrella module for the ecommerce family;
- the base split into `cart/customer/product/region/pricing/inventory/order/payment/fulfillment` is already done and deepening;
- a separate sales-channel domain in `commerce` is not needed: the platform already has `rustok-channel`, and ecommerce should become channel-aware over it, not duplicate its model.


## FFA transition (FBA deferred alignment track)

Status: `in progress`

> **Hard gate for new modules and large ecommerce slices:** a new ecommerce/marketplace module
> cannot be started as host-owned UI, ad-hoc REST/GraphQL handler or storage appendage inside
> `rustok-commerce`. First, lock module slug/ownership, canonical service contract,
> typed request context/errors, data ownership, explicit ports/events for cross-module dependencies
> and FFA/FBA status block in local docs + central registry. Only then add
> transport adapters (`#[server]`, GraphQL, REST/RPC) and module-owned UI as a thin adapter.

From this slice, the ecommerce roadmap is officially synchronized with the platform's transition to
Fluid Frontend Architecture (FFA) and Fluid Backend Architecture (FBA):

- FFA: module-owned UI surfaces (`admin`/`storefront`) remain the default path, and transport
  (`#[server]` + GraphQL/REST fallback) must maintain semantic parity without local
  divergence in domain logic;
- FBA: `rustok-commerce` and split ecommerce modules maintain a service-boundary-ready
  contract, where application services remain the canonical business core regardless of
  execution topology (embedded vs remote);
- the umbrella layer does not reclaim ownership of already separated bounded contexts and continues
  to act as the orchestration root for cross-domain scenarios checkout/post-order;
- all new Phase 8/9/10/11 increments must immediately pass FFA check and FBA-readiness guardrail:
  transport-neutral service semantics, channel-aware boundaries, typed context/error mapping,
  explicit module ports/events and absence of duplicated business rules in UI/transport adapters;
- before expanding the roadmap with new marketplace/provider modules, FBA evidence for already
  ready ecommerce slices must be brought to `in_progress -> boundary_ready candidate`: service contract first,
  transport adapters second, without host-owned business semantics.

Mandatory actions in upcoming iterations:

1. for each new ecommerce endpoint, lock FFA parity (`#[server]` ↔ GraphQL/REST);
2. for each new post-order scenario, lock FBA boundary evidence
   (service contract first, transport adapters second, typed context/errors, explicit ports/events);
3. for each new ecommerce/marketplace module before the first UI/transport PR, create
   a module-local `docs/implementation-plan.md` with FFA/FBA status block and a row in the central readiness board;
4. when updating the execution checkpoint, explicitly note which FFA invariants and FBA guardrails were
   verified in the specific slice.

## Scope of work

- keep `rustok-commerce` as the umbrella/root layer for the ecommerce family, not as the storage owner for already separated bounded contexts;
- evolve cross-domain orchestration, transport parity and channel-aware commerce contract over `rustok-channel`;
- complete UI split and further domain split without returning responsibility to the host layer or aggregate UI.

## Goals

- bring the Medusa-style ecommerce surface to production-grade level without locally invented semantics;
- keep GraphQL and REST over the same application services;
- stabilize checkout, cart context and orchestration between cart/payment/order/fulfillment;
- make commerce channel-aware over `rustok-channel`;
- fill in missing bounded contexts for Medusa parity: merchandising availability, pricing/promotions, tax, post-order flows and provider extensibility;
- maintain tenant isolation, outbox/event flow, index-backed read paths and the thin-host role of `apps/server`.

## Current priority for Medusa JS clone

The development order is locked as follows:

0. restore FBA-first stabilization gate for already ready ecommerce slices: lock boundary evidence
   for `product/cart/order/checkout/fulfillment/pricing/inventory` and post-order orchestration, remove
   host/transport-owned business semantics, describe explicit ports/events and only then expand
   the functional roadmap;
1. perform UI split by module ownership: move domain UI from aggregate `rustok-commerce-admin` / `rustok-commerce-storefront` to the corresponding split modules (`product`, `order`, `inventory`, `pricing`, `fulfillment`, `customer`, `region`), leaving `rustok-commerce` only orchestration/cross-domain surfaces;
2. continue `Phase 7` from already implemented seller-aware grouping and typed fulfillment-item model to post-order delivery changes;
3. move `Phase 8` to Pricing 2.0 with channel-aware price resolution, price lists, rules and promotions;
4. move `Phase 9` to a separate tax domain with tax lines and provider seam;
5. close `Phase 10` post-order surface (`returns/refunds/exchanges/claims/order changes`);
6. stabilize `Phase 11` provider architecture for payment/fulfillment only after FBA-readiness evidence for already ready payment/fulfillment/order boundaries;
7. lock `Phase 12` as Medusa parity matrix and release discipline;
8. after foundation + backfill + FBA gate, open a separate marketplace/seller-platform phase, rather than growing seller portal/RBAC/payouts inside umbrella `commerce`.

Nearest execution slice:

- first, continue the already started UI split: product admin route moved to `rustok-product/admin`, shipping-option admin route moved to `rustok-fulfillment/admin`, customer admin route moved to `rustok-customer/admin`, order admin route moved to `rustok-order/admin`, inventory admin route moved to `rustok-inventory/admin` with native set/adjust/reserve/release quantity and check-availability actions without GraphQL fallback, pricing admin route moved to `rustok-pricing/admin`, region admin route moved to `rustok-region/admin`, storefront split already underway through `rustok-region/storefront`, `rustok-product/storefront`, `rustok-pricing/storefront` and `rustok-cart/storefront`, and the aggregate `rustok-commerce-storefront` is already compressed to an aggregate checkout workspace, where seller-aware delivery-group shipping selection UI belongs to `rustok-fulfillment-storefront`, and commerce holds only a temporary transport callback;
- in parallel, lock `Marketplace Foundations`: canonical `seller_id` in product/cart/order/checkout/fulfillment contract, seller-aware grouping by `seller_id` and preparation of seller-owned read model without deploying the full marketplace feature set; marketplace/seller-platform surface opens only as a new FFA/FBA-first module boundary after foundation/backfill;
- `Phase 7` is now closed until explicit `reopen` / `reship` semantics over seller-aware grouping, typed fulfillment-item model, manual post-order create path and partial ship/deliver baseline;
- and now proceed to channel-aware pricing.

## Current state

- `rustok-commerce` already contains `CatalogService`, `PricingService`, `InventoryService`, `CheckoutService`, `StoreContextService`;
- storefront and admin REST routes live inside `crates/rustok-commerce/src/controllers/*`;
- GraphQL surface lives inside `crates/rustok-commerce/src/graphql/*`;
- cart snapshot already stores storefront context (`region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`, `customer_id`, `email`, `currency_code`) and channel snapshot (`channel_id`, `channel_slug`);
- checkout path uses `checking_out`, reuse payment collection and recovery semantics;
- checkout reuses pre-created cart payment collection, instead of creating a duplicate payment record at the `complete` step;
- guest checkout is allowed for guest cart without mandatory auth context, while customer-owned cart remains auth-gated;
- admin surface already has order/payment/fulfillment lifecycle transport and runtime parity with GraphQL;
- `apps/server` remains a thin host layer for route/OpenAPI/schema composition;
- `rustok-api` and `apps/server` already pass `ChannelContext` (`channel_id`, `channel_slug`, `channel_resolution_source`) in the request pipeline, and commerce storefront transport has already started using it for channel-aware gating, cart snapshot and order snapshot;
- legacy `/api/commerce/*` removed from the live router, OpenAPI and contract tests.

## What is explicitly missing

- full channel-aware publication and availability semantics for admin write-path, pricing/inventory/fulfillment and remaining commerce entities beyond storefront baseline;
- post-order delivery changes and item-level delivery recovery over the already introduced seller-aware deliverability baseline, typed fulfillment-item model, manual post-order create path and partial ship/deliver baseline;
- seller portal, merchant RBAC surfaces, commissions/payouts/settlement and disputes/returns marketplace-policy are consciously excluded from the nearest scope and are not part of the foundation slice;
- channel-aware price resolution is no longer pure backlog: `Phase 8` already received a host-channel-aware resolver foundation (`channel_id/channel_slug`, channel-scoped base rows and channel-filtered active price lists), but promotion/rule layering and full authoring UX still remain inside `Phase 8`;
- full promotion/discount domain over price rules, not only `compare_at_amount` and service-level `apply_discount`;
- separate tax domain: foundation already started (tax lines + totals), `rustok-tax` already moved default `region_default` calculation to a separate module boundary, and provider seam and typed `provider_id` tax-line contract are already introduced; the backlog now shifts to richer tax rules and external tax engines instead of continuing hardcoded region-only runtime;
- post-order layer at Medusa level: returns, exchanges, claims, order changes, draft/edit flows, refund transport;
- provider registry for payment/fulfillment, webhook ingestion and external gateway/carrier story; started guarded invocation seam (`execute_*`) in owner registries, but commerce orchestration is not yet migrated to production adapter wiring.

## Backlog contradictions

| ID | Contradiction | What needs to be done |
| --- | --- | --- |
| `BL-01` | umbrella module vs further split | continue moving stable bounded contexts to separate crates, leaving `rustok-commerce` as orchestration/root layer |
| `BL-02` | entities vs migrations vs indexer SQL | keep schema hardening, migration smoke and Postgres-first tests mandatory |
| `BL-03` | inventory model hardening | align read/write path around stock locations, levels, reservations, exported inventory-owned case-insensitive backorder policy helper and channel-aware availability |
| `BL-04` | transport parity vs domain completeness | do not confuse presence of `/store/*` and `/admin/*` transport with actual Medusa domain parity |
| `BL-05` | `/admin/*` and `/store/*` vs embedded UI routes | keep route precedence, OpenAPI and router smoke tests under constant regression |
| `BL-06` | Medusa parity scope | expand contract tests by official Medusa docs, not inventing local semantics |
| `BL-07` | platform `channel` already exists, but commerce remains channel-blind | make catalog/cart/order/pricing/inventory/fulfillment channel-aware over `rustok-channel`, without a second sales-channel layer |
| `BL-08` | pricing rows vs merchandising model | move from base prices and `compare_at_amount` to price lists, rules, tiers, adjustments and promotions |
| `BL-09` | region tax flags vs separate tax domain | move tax calculation/rules/providers from flat `region` model to a separate bounded context |
| `BL-10` | linear order lifecycle vs post-order reality | add returns, refunds, exchanges, claims, order changes and draft/edit semantics |
| `BL-11` | manual/default providers vs extensibility | stabilize payment/fulfillment provider SPI instead of mixing base model with external integrations |
| `BL-12` | typed shipping profile registry, typed product/variant bindings, seller-aware line-item snapshots, cart delivery groups, multi-fulfillment checkout and typed fulfillment items already exist, but deliverability model is not yet closed to post-order level | bring deliverability domain from seller-aware grouping and typed fulfillment items to post-order delivery changes |
| `BL-13` | split backend already exists, but storefront and part of admin ownership are still aggregated in `rustok-commerce-storefront` and remaining umbrella admin routes | distribute admin/storefront UI across split ecommerce modules and leave umbrella UI only for cross-domain orchestration surfaces |
| `BL-14` | seller-aware grouping already exists, but the marketplace identity boundary historically relied on `seller_scope` instead of a stable opaque key | lock `seller_id` as canonical multivendor boundary in product/cart/order/fulfillment contracts and clean up runtime fallback to `seller_scope` |

## Stages

### Phase 1. Module topology and contracts

Status: `done`

- `rustok-commerce` locked as umbrella/root module;
- base split into profile crates is complete;
- shared DTO/entities/errors moved to `rustok-commerce-foundation`.

### Phase 1.5. UI split by ownership boundaries

Status: `in progress`

What is already closed in the current slice:

- `rustok-product` already publishes its own module-owned admin UI package `rustok-product/admin`;
- `rustok-product/rustok-module.toml` already exports `[provides.admin_ui]`, and `apps/admin` picks up the new route through manifest-driven composition;
- `rustok-product` has already taken product CRUD ownership;
- `rustok-product` already publishes its own module-owned storefront UI package `rustok-product/storefront` and uses native Leptos server functions as the default internal data layer with GraphQL fallback;
- `rustok-pricing` already publishes its own module-owned storefront UI package `rustok-pricing/storefront` and uses native Leptos server functions as the default internal data layer with GraphQL fallback;
- `rustok-fulfillment` already publishes its own module-owned admin UI package `rustok-fulfillment/admin`;
- `rustok-customer` already publishes its own module-owned admin UI package `rustok-customer/admin` and uses native Leptos server functions as the default admin data layer;
- `rustok-region` already publishes its own module-owned admin UI package `rustok-region/admin` and uses native Leptos server functions as the default admin data layer;
- `rustok-region` already publishes its own module-owned storefront UI package `rustok-region/storefront` and uses native Leptos server functions as the default internal data layer with GraphQL fallback;
- `rustok-order` already publishes its own module-owned admin UI package `rustok-order/admin`;
- `rustok-inventory` already publishes its own module-owned admin UI package `rustok-inventory/admin` for stock visibility and low-stock triage;
- `rustok-pricing` already publishes its own module-owned admin UI package `rustok-pricing/admin` for price visibility, sale markers and currency coverage;
- `rustok-pricing` already publishes its own module-owned storefront UI package `rustok-pricing/storefront` for public pricing atlas, sale markers and currency coverage;
- `rustok-cart` already publishes its own module-owned storefront UI package `rustok-cart/storefront` for storefront cart inspection, safe decrement/remove line-item actions and seller-aware delivery-group snapshot;
- aggregate `rustok-commerce-storefront` no longer holds the published catalog/pricing read-side and is compressed to an aggregate checkout workspace with request/tenant/channel context summary, seller-aware delivery-group shipping selection, `payment collection` reuse and `complete checkout` actions by `?cart_id=`;
- aggregate `rustok-commerce-admin` no longer duplicates product/shipping-option flows and is left only under shipping profiles.

Next steps:

- [x] move region UI by module ownership boundary;
- [x] start separate storefront split through `rustok-region/storefront`;
- [x] move product storefront read-side from aggregate `rustok-commerce-storefront`;
- [x] move pricing storefront read-side from aggregate `rustok-commerce-storefront`.
- [x] compress `rustok-commerce-storefront` to aggregate checkout workspace without catalog/pricing ownership.
- [x] move cart storefront inspection read-side to `rustok-cart/storefront`.
- [x] leave only aggregate checkout workspace in `rustok-commerce-storefront` for delivery-group shipping selection, `payment collection` and `complete checkout`.

### Phase 2. Medusa-style transport baseline

Status: `done`

- live REST surface is up at `/store/*` and `/admin/*`;
- storefront routes `products`, `regions`, `shipping-options`, `carts`, `payment-collections`, `orders/{id}`, `customers/me` implemented;
- admin routes for `products` implemented;
- OpenAPI and route contract tests are tied to live surface without legacy compatibility layer.


### Inventory availability compatibility tail

Status: `in progress`

This tail belongs to umbrella ecommerce orchestration, not to the inventory admin UI scope.
`rustok-inventory` already owns the admin read/write facade and public-channel inventory
availability/projection helpers; `rustok-commerce` should remain a thin compatibility
layer for storefront/checkout flows.

Current rules:

- GraphQL cart line quantity mutations, checkout cart inventory validation and store REST cart
  validation should call `rustok_inventory::check_variant_availability_for_public_channel`
  instead of directly coupling backorder policy + channel-visible inventory loader.
- Storefront product DTO projection should call
  `rustok_inventory::load_inventory_projection_by_variant_for_public_channel` and only
  apply `PublicChannelInventoryProjection.available_quantity/in_stock` to commerce DTO.
- Commerce callers should not directly call
  `load_available_inventory_for_variant_in_public_channel`,
  `load_available_inventory_by_variant_for_public_channel` or
  `inventory_policy_allows_backorder` for storefront availability decisions.
- Further work on stock locations, reservations and channel-aware availability edge-cases
  is captured here as umbrella compatibility/parity work and should be accompanied by
  integration tests for checkout/catalog visibility flows.

Fast guardrails:

```bash
node scripts/verify/verify-inventory-admin-boundary.mjs
./scripts/verify/verify-all.sh inventory-admin-boundary
```

Next steps:

- [ ] add targeted integration coverage for channel-aware inventory visibility edge-cases
  through storefront catalog/cart/checkout path;
- [ ] maintain REST/GraphQL/native parity for checkout-facing inventory availability
  after expanding stock locations/reservation semantics;
- [ ] when changing public-channel inventory semantics, synchronize
  `crates/rustok-inventory/docs/implementation-plan.md`, this commerce roadmap and
  `docs/modules/registry.md`.

### Phase 3. Cart context and checkout hardening

Status: `in progress`

Focus:

- keep cart as the source of truth for storefront context;
- evolve checkout recovery/idempotency semantics;
- close race conditions on `payment-collections` and `complete checkout`;
- keep transport response shape stable;
- lock cart model as storefront source of truth, including channel snapshot, without breaking the API again.

Mandatory checks:

- migration tests for cart context schema;
- integration tests `create cart -> update context -> add line item -> shipping options -> payment collection -> complete`;
- negative tests on `currency_code` vs `region_id`;
- auth/customer ownership tests;
- contract tests for store cart endpoints;
- regression tests on repeated `complete checkout` and reuse of existing payment collection.

What is already closed in the current slice:

- transport coverage confirms that cart context remains the source of truth for `shipping-options`, `payment-collections` and `checkout`;
- transport coverage closes `currency_code` vs `region_id`, guest/customer ownership and the end-to-end storefront checkout flow;
- service coverage confirms reuse of already existing cart-bound payment collection during `complete checkout`.
- cart/order transport now preserves channel snapshot and uses it as part of storefront context during checkout.
- storefront payment-collection and complete-checkout paths reprice cart line items before creating payment collection and before `complete checkout`, so price-list/quantity-tier changes do not leave a stale pricing snapshot: line items with discounts normalize to `base/compare_at unit_price` plus pricing-owned `cart_adjustments`, and the payment collection continues to use the net `cart.total_amount`.
- cart/order totals are now supplemented with first-class `shipping_total`: selected shipping options are included
  in the persisted `cart.total_amount`, checkout snapshots `shipping_total` into order, and the payment collection
  no longer lives on subtotal-minus-adjustments without delivery.
- Over this base contract, `rustok-cart` has already started a typed shipping-promotion layer: percentage/fixed
  shipping discounts live in `cart_adjustments` as `scope=shipping`, checkout snapshots them into order,
  and the payment collection remains tied to the same net total without hidden fallback to the old semantics.
- Cart checkout lifecycle guardrails are additionally locked at the service coverage level: `checking_out` carts
  reject mutation paths for typed promotions and generic adjustment writes, `release_checkout`
  restores permissible mutations without setting `completed_at`, and `complete_cart` leaves
  the cart in the final `completed` state without repeated checkout/release.
- Umbrella-level regression expectations are synchronized with the cart boundary: checkout recovery in `rustok-commerce` must preserve visibility of these guardrails at the transport/service level (re-entry/release/complete invariants cannot be hidden only in the cart module's unit coverage).

### Phase 4. Order/payment/fulfillment transport

Status: `in progress`

Focus:

- expand admin/store transport over already separated modules;
- lock response shape and lifecycle semantics;
- continue parity between REST and GraphQL over shared services;
- do not consider the phase closed while post-order scenarios are still excluded.

What is already closed in the current slice:

- added admin order transport endpoint `GET /admin/orders/{id}`;
- added paginated admin orders list endpoint `GET /admin/orders` with base filters `status` and `customer_id`;
- admin order detail returns order together with the latest payment collection and latest fulfillment;
- added explicit admin order lifecycle endpoints: `mark-paid`, `ship`, `deliver`, `cancel`;
- added admin list/detail/lifecycle endpoints for `payment-collections` (`list`, `show`, `authorize`, `capture`, `cancel`) and `fulfillments` (`list`, `show`, `ship`, `deliver`, `cancel`);
- transport/OpenAPI coverage locks RBAC and schema contract for admin order detail and admin payment/fulfillment lifecycle surface;
- GraphQL parity expanded to admin order/payment/fulfillment surface: read queries (`order`, `orders`, `paymentCollection`, `paymentCollections`, `fulfillment`, `fulfillments`) and lifecycle mutations now work over the same `OrderService`/`PaymentService`/`FulfillmentService` as REST, and are covered by a runtime parity test;
- storefront GraphQL read parity covers `storefrontMe` and `storefrontOrder`, including ownership guard for another customer's order;
- storefront GraphQL mutation surface covers `createStorefrontPaymentCollection` and `completeStorefrontCheckout`, including guest checkout and reuse of already created cart-bound payment collection;
- storefront GraphQL cart surface covers `storefrontCart`, `createStorefrontCart`, line-item lifecycle and tri-state patch semantics for cart context;
- storefront GraphQL discovery/read surface includes `storefrontRegions` and `storefrontShippingOptions`, including cart-context precedence over conflicting query currency; additionally, the storefront facade now returns `storefrontPricingChannels`, `storefrontActivePriceLists(channelId, channelSlug)`, `storefrontPricingProduct` and `adminPricingProduct`, so the module-owned pricing fallback does not lose either channel-aware selector parity or variant-level `effective_price` parity.
- The admin GraphQL facade now also holds pricing write mutations for
  `updateAdminPricingVariantPrice`, `previewAdminPricingVariantDiscount` and
  `applyAdminPricingVariantDiscount`, so the pricing-owned admin write path has
  not only native `#[server]`, but also parallel GraphQL transport over the same
  `PricingService`.
- Generic catalog roots `product` / `storefrontProduct` are meanwhile locked as the catalog-authoritative surface, and their `variants.prices` remains only a compatibility snapshot without pricing-authoritative contract status.

### Phase 5. Umbrella module simplification

Status: `in progress`

Focus:

- remove dead transport, compatibility remnants and duplicate code without regard for a non-existent migration period;
- keep `rustok-commerce` as the orchestration/root layer, not a warehouse for historical adapters;
- move remaining stable areas to profile crates;
- do not pull domain logic back into `apps/server`.

What is already done:

- legacy REST surface `/api/commerce/*` removed;
- rollout/deprecation middleware, settings, runtime guardrails and operator scripts that only made sense for legacy cutover removed;
- OpenAPI and route tests migrated to live `/store/*` + `/admin/*` contract.

### Phase 6. Commerce channel-awareness

Status: `in progress`

Focus:

- use the existing `rustok-channel` as a platform-level delivery context;
- make catalog, cart, order, inventory and fulfillment channel-aware without creating a second sales-channel domain;
- link publication/availability semantics of commerce with channel bindings and `ChannelContext`.

What is already started in the current slice:

- cart received `cart_tax_lines`, `tax_total` and tax recalculation over line items + selected shipping options;
- order received `order_tax_lines`, `tax_total`, `tax_included` and snapshots tax lines at checkout/create-order;
- tax-inclusive vs tax-exclusive semantics is captured in metadata tax line (`tax_included`);
- REST/GraphQL/Leptos DTO for cart/order have started returning tax lines and totals.

Deliverables:

- channel-aware product publication and catalog visibility;
- `channel_id` as part of cart/order snapshot and read-model filtering where needed by the domain;
- channel-aware selection for shipping options and stock availability;
- explicit precedence rules between `channel`, `region`, `currency` and tenant locale policy.

What is already closed in the current slice:

- storefront REST and storefront GraphQL now stop at the request channel if the commerce module is not enabled for it through `channel_module_bindings`;
- catalog read-path (`/store/products`, `storefrontProduct`, `storefrontProducts`) already filters products by metadata-based allowlist on `channel_slug`, over the base check of `active + published`;
- shipping options in REST/GraphQL and checkout validation already respect the same channel visibility semantics, with cart `channel_slug` having precedence over conflicting request/query context;
- cart line-item mutations no longer accept products hidden for the current storefront channel;
- storefront product detail and cart line-item quantity checks now calculate available inventory only from stock locations visible to the current storefront channel;
- checkout service now re-validates cart line items against current product visibility and channel-visible inventory, so a stale cart does not complete into an order with a hidden product or already unavailable stock;
- channel-aware price resolution is consciously not considered fully closed in this phase: the foundation has already moved to `Phase 8`, but promotion/rule layering and authoring UX are still ongoing there, to avoid mixing storefront availability with pricing 2.0;
- transport and service tests already cover disabled channel module, hidden products, hidden shipping options and checkout reject path for channel-hidden shipping option.

Mandatory checks:

- integration tests on `ChannelContext -> catalog/cart/checkout`;
- negative tests on an inactive or unbound channel;
- regression tests on absence of a second local sales-channel layer;
- docs sync with `rustok-channel` if contracts between modules change.

### Phase 7. Deliverability domain and split fulfillment

Status: `in progress`

Focus:

- close the gap between catalog and fulfillment boundary not only at the level of compatibility rules, but also at the cart/order/fulfillment model level;
- lock effective shipping profile as a typed domain concept: `variant -> product -> default`;
- separate the deliverability domain from the old single-option cart semantics.

Deliverables:

- typed `shipping_profile_slug` for product and variant + effective-profile resolution;
- typed line-item snapshot `shipping_profile_slug` in cart/order;
- typed cart `shipping_selections`, `delivery_groups[]` and multi-fulfillment checkout;
- compatibility shims for single-group carts through legacy `selected_shipping_option_id`, `shipping_option_id` and `fulfillment`.

What is already closed in the current slice:

- metadata-backed baseline is no longer the only source of truth: schema/migration `shipping_profiles`, typed `ShippingProfileService` and admin-facing registry for shipping profiles are introduced;
- product create/update/read contracts already expose first-class `shipping_profile_slug`, shipping option read/create contracts expose first-class `allowed_shipping_profile_slugs`, and `products.shipping_profile_slug` and `product_variants.shipping_profile_slug` now exist as typed persistence;
- admin REST/GraphQL surface already supports `list/show/create/update/deactivate/reactivate` shipping options with typed `allowed_shipping_profile_slugs`, so shipping profile compatibility and lifecycle no longer live only in service/tests;
- admin REST/GraphQL surface now also supports `list/show/create/update/deactivate/reactivate` shipping profiles, and product/shipping-option write-path validates references against the active registry;
- module-owned `rustok-commerce/admin` UI already consumes this control plane directly and can show inactive shipping options together with explicit lifecycle actions;
- `CatalogService` and `FulfillmentService` still normalize these fields into metadata-backed storage shape for backward compatibility, but the source of truth for deliverability decisions already lives in typed product/variant fields, typed registry and line-item snapshots;
- product create/update/read contracts now also accept nullable `seller_id`, so seller identity comes from the catalog write-side, not computed from merchandising/display fields like `vendor`;
- cart line items now store effective `shipping_profile_slug` and canonical seller snapshot (`seller_id`), cart response returns seller-aware `delivery_groups[]`, and store/GraphQL checkout input accepts typed `shipping_selections[]`;
- `cart_shipping_selections` now persists seller-aware key by `(shipping_profile_slug, seller_id)` and does not fall back to `seller_scope`;
- `CheckoutService` now validates stale shipping-profile snapshots, removes incompatible selections by delivery groups and creates a separate fulfillment for each delivery group;
- storefront REST/GraphQL and module-owned storefront UI packages (`rustok-cart/storefront`, `rustok-commerce/storefront`) now pass seller-aware delivery-group contract end-to-end with canonical `seller_id`, and fulfillment metadata preserves language-agnostic seller identity without seller display text;
- fulfillment boundary now stores typed `fulfillment_items` and checkout links delivery groups with `order_line_item_id`, so item scope is no longer held only on `delivery_group.line_item_ids` inside metadata;
- admin REST/GraphQL now also support manual post-order `create fulfillment` with typed `items[]`: create path validates `order_line_item_id` against the order, remaining quantity against already created non-cancelled fulfillments and prevents mixing different seller-aware delivery groups in one follow-up fulfillment;
- `FulfillmentService` now holds item-level `shipped_quantity` / `delivered_quantity`, and admin REST/GraphQL `ship` / `deliver` accept optional quantity adjustments by `fulfillment_item_id`, so partial post-order delivery progress and audit trail live in the typed fulfillment boundary;
- admin REST/GraphQL now already support explicit `reopen` / `reship` for fulfilled/cancelled recovery path: delivered fulfillments can be returned to `shipped`, cancelled fulfillments can be returned to actionable state, and delivery corrections no longer require implicit status hacks;
- `CompleteCheckoutResponse` and storefront GraphQL checkout surface now return `fulfillments[]`, while singular `fulfillment` remains only as a compatibility shim for single-group carts;
- the previous strict single-option mixed-cart invariant is no longer the target architecture: it is preserved only as a compatibility shortcut for carts with one delivery group;
- regression tests already cover effective-profile resolution, mixed-cart delivery groups, missing per-group selection, multi-fulfillment checkout, stale snapshot reject path and GraphQL checkout parity for new fields.

Mandatory checks:

- contract tests on incompatible products and shipping options;
- migration tests for product/variant/cart/order shipping-profile schema;
- integration tests on mixed cart with different fulfillment policy;
- regression tests on preflight checkout failures, which should release the `checking_out` lock and not create payment/order artifacts before side effects.

### Cross-cutting. Marketplace Foundations

Status: `in progress`

Focus:

- add minimal multivendor foundation without deploying seller portal, payouts, commissions or disputes;
- lock `seller_id` as canonical seller identity key for ecommerce write-side and orchestration;
- do not store seller display label in ecommerce storage and do not use `vendor` as seller identity.

What is already closed:

- product create/update/read contracts now include nullable `seller_id`;
- cart line items, `cart_shipping_selections`, order line items and fulfillment delivery-group metadata now carry `seller_id` as canonical key;
- cart grouping, checkout fulfillment metadata and manual fulfillment validation now rely on `(shipping_profile_slug, seller_id)` without fallback to `seller_scope`;
- typed `cart_adjustments` and `order_adjustments` lock promotion/discount snapshot as language-neutral business data: source identity is stored through `source_type/source_id`, amounts through `amount/currency_code`, and display label does not enter ecommerce storage;
- REST, GraphQL and Leptos `#[server]` contracts for product/cart/checkout/manual fulfillment are already expanded with the `seller_id` field;
- seller display label is no longer persisted in ecommerce storage, and `seller_scope` is no longer used as a runtime identity fallback in cart grouping/selection or manual fulfillment metadata.

Next steps:

- prepare seller-owned read model/resolver for display label by `seller_id` and effective locale;
- continue cleaning remaining storage/API references to `seller_scope` in separate atomic owner-module slices without runtime fallback;
- before seller portal, merchant RBAC, commissions, payouts, settlement and disputes, create a separate marketplace/seller-platform module plan with FFA/FBA status block, canonical service contract, data ownership, typed context/errors, explicit ports/events and transport parity DoD;
- do not expand the current scope into merchant RBAC, seller portal, payouts, commissions and disputes until foundation + backfill + FBA-readiness gate for already ready ecommerce boundaries is complete.

### Phase 8. Pricing 2.0 and promotions

Status: `in progress`

Focus:

- move beyond `prices.amount` / `compare_at_amount` / service-level `apply_discount`;
- add price lists, rules, tiers and adjustments;
- move promotions to a separate bounded context instead of implicit price mutation.

Deliverables:

- pricing context `channel + region + currency + customer segment` where it is actually needed;
- price lists and rule-driven resolution;
- typed cart/order adjustments as a separate business snapshot layer, not mixed with base price rows, price-list rows or localized display metadata;
- promotion engine for item/order/shipping discounts without mixing with base price storage.

What is already started in the current slice:

- `rustok-pricing::PricingService` already received a typed resolver foundation
  `PriceResolutionContext -> ResolvedPrice` over base-price rows;
- pricing resolution contract is already hardened: `currency_code` is validated as
  three-letter ASCII business code, `quantity < 1` is rejected, and GraphQL roots
  `adminPricingProduct` / `storefrontPricingProduct` do not accept `region_id`,
  `price_list_id` or `quantity` without explicit `currencyCode`;
- active precedence is already deterministic: exact `region_id` takes priority over global price,
  quantity tiers are selected by the more specific `min_quantity` and narrower `max_quantity`;
- explicit active `price_list_id` overlay is already activated in the resolver, and
  the module-owned pricing admin/storefront surfaces already received pricing-owned
  active price-list selector over this read contract.
- `rustok-pricing/admin` is no longer a purely read-only surface: the module-owned
  server-function transport already covers base-price updates on variant prices,
  active `price_list` overrides and rule/scope editing for active price lists.
- quantity tiers now also received a minimal write path in `rustok-pricing/admin`:
  the operator can set `min_quantity` / `max_quantity` for variant price rows, and the
  resolver immediately uses these windows for effective-price selection.
- The same module-owned admin write path now also supports active `price_list_id`
  overrides over base prices, and transport parity is covered by SSR tests on happy path and permission gate.
- Over this, the pricing runtime now returns typed `discount_percent` in the resolved/effective
  price contract, and the module-owned admin/storefront surfaces show sale math without ad-hoc
  calculation only from `compare_at_amount`.
- Parallel admin GraphQL transport now also covers not just base-row writes:
  `updateAdminPricingVariantPrice`, `previewAdminPricingVariantDiscount`,
  `applyAdminPricingVariantDiscount`, `updateAdminPricingPriceListRule` and
  `updateAdminPricingPriceListScope` work over the same `PricingService`,
  preserving lifecycle/scope parity with native pricing admin transport.
- The legacy service-level `apply_discount` has also started shrinking to a compatibility layer:
  typed percentage-adjustment preview/apply path now lives inside `rustok-pricing` and
  works on canonical base-price row or on the selected active `price_list` override.
- Targeted transport parity for this admin write path is already noticeably expanded: `rustok-pricing/admin`
  has SSR coverage not only on native `update-variant-price`, but also on
  rule/scope lifecycle, inactive time-window guards and channel mismatch without hidden fallback.
- Over this, `rustok-pricing/admin` already received a module-owned operator flow for typed
  percentage-discount preview/apply on canonical base row; now the same flow can also
  target the selected active `price_list` override, and SSR tests cover not only raw
  price row updates, but also admin transport parity for the adjustment path.
- The next promotion-ready layer is also already started inside the pricing boundary: active `price_list`
  can now hold a typed percentage rule, `PricingService::resolve_variant_price`
  can fall back to the base row through this rule when no explicit override row exists,
  and `rustok-pricing/admin` already supports editing this rule through module-owned server functions.
- Pricing-focused GraphQL/runtime parity is also already expanded: `adminPricingProduct`,
  `storefrontPricingProduct` and active price-list selectors pass the full parity sweep
  together with the rest of `graphql_runtime_parity_test`, and clear/scope updates do not leave
  stale selector metadata.
- Cart/order promotion representation now also has a typed foundation: `rustok-cart` stores `cart_adjustments`,
  recalculates `subtotal_amount`, `adjustment_total` and net `total_amount`, and `rustok-order` snapshots
  `order_adjustments` at checkout/create-order and returns the same summary in REST/GraphQL/Leptos-facing DTO;
  this layer does not store seller/product/promotion display labels and remains resilient to default locale changes;
  storefront repricing under discount now captures in the line item not the effective sale price, but `base/compare_at`
  `unit_price`, while discount savings live in the typed adjustment snapshot.
- Storefront/admin GraphQL parity now also covers this snapshot layer: storefront cart/query + checkout
  preserve typed `adjustments`, payment collection uses net `cart.total_amount`, and the completed order
  carries sanitized adjustment metadata without `display_label`.
- Storefront/admin REST transport now also covers this snapshot layer: controller tests for
  `/store/carts/{id}` and `/admin/orders/{id}` lock typed `adjustments`, sanitized metadata and
  the current shipping-selection semantics, where an incompatible selection can soft-clear to `null`,
  and the verification baseline for the umbrella module again includes full `cargo test -p rustok-commerce --lib`.
- Storefront GraphQL add-to-cart now resolves `unit_price` through `PricingService` with the same
  `PriceResolutionContext` (currency + region + channel + quantity), rather than through the raw `price` row;
  this aligns pricing semantics between REST and GraphQL storefront cart path and provides a common
  `base unit_price + pricing adjustment` snapshot contract; the add-to-cart write path now
  writes this snapshot atomically in a single cart transaction, not through a separate follow-up repricing step.
- Storefront cart quantity update now reevaluates line items through the pricing resolver,
  so quantity tiers and channel-aware pricing apply on quantity change without writing
  the effective sale price directly into the persisted `unit_price`.
- Storefront cart context update (region/country/locale/shipping selections) now
  reprices all line items through the pricing resolver, so a context change does not leave a
  stale pricing snapshot and reassembles `base unit_price + adjustments` under the new storefront context.
- A typed promotion runtime over the snapshot layer is also already started in `rustok-cart`: the cart service can
  preview/apply percentage/fixed promotions on cart-level and line-item scope, without overwriting pricing-owned
  adjustments and preserving order/payment snapshot parity through the existing checkout flow.
- Operator-side GraphQL transport over this runtime is also already present: admin mutations can preview/apply
  typed cart promotions for `cart`, `line_item` and `shipping` scope, using the same `CartService`
  instead of a separate promotion-specific storage or ad hoc adjustment writer.
- Native-first operator transport over the same runtime is also already present in `rustok-commerce-admin`:
  package-level `#[server]` functions can preview/apply typed cart promotions for `cart`,
  `line_item` and `shipping` scope, use the same `CartService`, hold the same permission contract
  (`orders:read` for preview, `orders:update` for apply) and are covered by SSR tests on shipping scope,
  target validation and permission gate.

Mandatory checks:

- deterministic price-resolution tests;
- contract tests on priority/override semantics;
- cart/order adjustment snapshot tests: net total, source identity, line-item binding and absence of localized display labels in storage;
- checkout regression tests: cart adjustments should snapshot into order adjustments, and payment collection should use net `cart.total_amount`;
- regression tests on rounding and decimal money contract;
- transport tests on price + promotion representation in `/store/*`, `/admin/*` and GraphQL,
  including storefront parity for `shipping_total` and shipping-scoped promotion snapshot.

### Phase 9. Tax domain

Status: `in progress`

Focus:

- stop considering `region.tax_rate` and `region.tax_included` as a sufficient tax model;
- introduce a separate tax bounded context with tax lines, rules and provider seam;
- do not break the current checkout flow during gradual migration.

Deliverables:

- tax calculation context over cart/order/shipping;
- tax lines for line items and shipping;
- provider seam for external tax engines;
- migration path from flat region tax policy to a more realistic model.

What is already started in the current slice:

- `rustok-tax` introduced as a separate bounded context for tax calculation contract instead of continuing hardcoded tax runtime inside `rustok-cart`;
- default provider `region_default` preserves the current semantics of `region.tax_rate` / `tax_included`, but now lives behind a provider seam;
- the current provider selection hook goes through `regions.tax_provider_id`; an unknown provider is cut off as a validation error in the cart runtime instead of a hidden fallback;
- `rustok-cart` no longer calculates tax lines directly from region helper code: the cart runtime calls `TaxService` and snapshots provider-aware tax lines;
- `cart_tax_lines` and `order_tax_lines` now carry first-class `provider_id`, and checkout transfers this snapshot to order without a hidden metadata-only fallback;
- targeted regression already locks that complete checkout preserves `provider_id=region_default` in cart/order tax lines together with `tax_included` metadata.
- The cart runtime now also accounts for channel-aware provider mapping from region metadata key `channel_tax_provider_ids`: when `cart.channel_id` is present, the tax pipeline passes `channel_provider_id` to `TaxService` with precedence over region `tax_provider_id`.

Mandatory checks:

- integration tests `cart -> taxes -> payment -> order`;
- negative tests on conflict of tax-inclusive/exclusive semantics;
- contract tests on transport shape of tax lines.

Next execution slice (coding plan continuation):

- [x] add channel-aware provider mapping (`regions.tax_provider_id` + `channel_id`) without hidden fallback to `region_default`;
- [x] expand `rustok-tax` to typed rule input (`item class`, `shipping class`, `customer tax-exempt`) without returning tax logic to `rustok-cart`;
- [x] lock admin/store read-side tax breakdown contract (line-item vs shipping vs order aggregate) in REST and GraphQL parity tests;
- [x] add migration/contract smoke for backfill of `provider_id` in legacy `order_tax_lines` snapshots.

### Phase 10. Post-order flows: returns, refunds, exchanges, claims, order changes

Status: `in progress`

Focus:

- move beyond the linear `pending -> confirmed -> paid -> shipped -> delivered/cancelled`;
- make refund/return semantics part of the domain, not just a state-machine helper;
- add order-change/draft-edit layer needed for Medusa-style OMS behavior.

Deliverables:

- return/refund records and lifecycle;
- exchanges / claims as order-change-backed post-order decisions within the target Medusa parity scope;
- order change / draft order / preview-apply semantics;
- admin/store transport for post-order scenarios.

Current state:

- starting refund slice already deployed over `payment-collections`: `rustok-payment` now stores first-class `refunds`, `PaymentService` supports `create/list/show/complete/cancel`, and the aggregate `PaymentCollectionResponse` returns `refunded_amount` and `refunds[]`;
- admin REST/GraphQL already publish the first post-order refund transport (`/admin/payment-collections/{id}/refunds`, `/admin/refunds*`, `createRefund`, `completeRefund`, `cancelRefund`, `refunds`), so Phase 10 no longer starts from zero;
- the remaining volume inside Phase 10 is broader than the refund-only baseline: returns, exchanges/claims and order-change/draft-edit semantics;
- claims scope decision is locked without a separate storage owner in `rustok-commerce`: a claim decision creates an order-owned `order_change` with `change_type=claim`, completes the return as `resolution_type=claim` with `order_change_id` and leaves further lifecycle in `rustok-order`.


Execution slices (Phase 10):

- [x] Slice 10.1: returns foundation (`rustok-order` storage + service lifecycle + admin REST/GraphQL read/write transport). Storage/read baseline was started earlier; this slice added show/read, complete/cancel lifecycle, REST routes `/admin/returns/{id}`, `/admin/returns/{id}/complete`, `/admin/returns/{id}/cancel`, GraphQL `orderReturn(s)` + `create/complete/cancelOrderReturn`, OpenAPI registration and targeted lifecycle tests. Item-level return lines closed in this slice via `order_return_items`; added resolution references of completed return (`resolution_type/refund_id/order_change_id`), and the umbrella complete-return REST/GraphQL helper creates/optionally completes refund via `PaymentService` and passes `refund_id`; exchange and claim helpers are also automated.
- [x] Slice 10.2: refund transport parity expansion (store/customer-safe read-side + ownership/RBAC contract tests).
- [x] Slice 10.3: order-change groundwork (draft edit snapshot + preview/apply contract skeleton without host-owned logic). Started in `rustok-order`: `order_changes` storage/service skeleton with `pending -> applied|cancelled` lifecycle and service tests. This slice added umbrella admin REST routes `/admin/orders/{id}/changes`, `/admin/order-changes*`, lifecycle routes `apply/cancel`, OpenAPI contract registration and GraphQL parity roots `orderChange(s)` + mutations `create/apply/cancelOrderChange`; further: storefront customer-facing read-side `GET /store/orders/{id}/changes` + GraphQL `storefrontOrderChanges` with customer ownership guard closed; linking of changes with refund/exchange orchestration closed via `PostOrderOrchestrationService.apply_exchange_order_change` / `apply_claim_order_change`.
- [x] Slice 10.4: exchanges/claims scope decision + parity matrix update in this plan and module docs. Decision tree brought to transport-level parity: admin REST `POST /admin/orders/{id}/returns/decision` and admin GraphQL `createOrderReturnDecision` use the same `PostOrderOrchestrationService` and publish unified `ReturnDecisionResponse` (`return_only/refund/exchange/claim`) without host-owned logic. Claims scope decision fixed as order-change-backed claim (`change_type=claim`) with `order_return_id` in preview/metadata and completed return `resolution_type=claim/order_change_id`; live REST and GraphQL runtime parity additionally check claim response output (`order_return/orderReturn`, `order_change/orderChange`, `refund=null`) against runtime service semantics. Dedicated claim storage/API remains out of scope until a dedicated bounded context is introduced. The UX slice added in `rustok-commerce-admin` a post-order change operator for `orderChanges` with `apply/cancel` actions, and the next increment transitioned this operator to native-first `#[server]` API over `OrderService` with SSR tests on pending filter, apply/cancel lifecycle, and permission gates; GraphQL fallback is kept for unavailable native transport. Exchange/claim helper metadata also marks created order changes `return_decision_action` / `return_decision_source`, and operator UI displays resolution summary cards from order-change preview/metadata without moving domain rules to host.

Mandatory checks:

- state-machine and property tests for refund/return/order-change transitions;
- RBAC/ownership tests for customer/admin post-order flows;
- contract tests against live transport for refund/return/order-change surface.

### Phase 11. Provider architecture

Status: `in progress`

Focus:

- do not mix manual/default payment/fulfillment domain model with provider-specific code;
- first stabilize the SPI, then connect specific gateway/carrier integrations;
- keep `rustok-commerce` as the orchestration layer, not a place for vendor-specific adapters.

Deliverables:

- [x] payment provider SPI baseline: manual provider descriptor/capabilities and adapter boundary for authorize/capture/cancel/refund;
- [x] fulfillment provider SPI baseline: manual carrier descriptor/capabilities and adapter boundary for rate quote/create label/cancel;
- [x] payment provider SPI static evidence, typed webhook adapter operation and webhook ingress/replay contract;
- [x] fulfillment provider SPI static evidence, typed carrier webhook adapter operation and carrier webhook replay contract;
- [x] external provider/carrier adapter registration static contract locks (descriptor/capability match, health/degraded-mode mapping, no lifecycle persistence in adapters);
- [x] owner registry runtime-mode guardrails plus no-compile runtime-smoke packets for operation capability rejection, missing-provider errors, registration failures and degraded-mode fallback mapping before external adapter invocation;
- [x] provider capability model baseline for authorize/capture/refund/cancel and rate-quote/label/cancel;
- [x] explicit fallback semantics for manual/default providers;
- [x] no-compile external gateway/carrier registration failure and remote degraded/unavailable runtime-mode smoke evidence.
- [x] no-compile live-adapter execution contract packets for payment/fulfillment gateway/carrier runtime cases.
- [x] owner registry guarded async invocation seams (`execute_*`) for payment/fulfillment adapter calls source-locked by the provider SPI verifier.
- [x] live external gateway/carrier adapter registration and remote failure/degraded-mode execution evidence.
- [x] commerce post-checkout payment runtime wiring: admin REST/GraphQL cancel/refund creation invoke payment-owner `execute_cancel` / `execute_refund` registry seams before lifecycle persistence, with no-compile verifier source locks.

Mandatory checks:

- static provider SPI evidence verifier (`scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`) and runtime provider SPI contract tests;
- replay/idempotency tests for webhooks;
- negative tests on partially successful external operations.

### Phase 12. Parity matrix and release discipline

Status: `planned`

Focus:

- translate the roadmap from a set of local features into an explicit Medusa parity matrix;
- lock `feature -> module -> transport -> tests -> status`;
- do not release transport as "ready" if the domain layer under it is still incomplete.

Deliverables:

- parity matrix by official Medusa docs;
- release checklist for `/store/*`, `/admin/*` and GraphQL parity;
- list of consciously deferred features with explicit explanation of why they are outside the current scope.

## Verification

Mandatory minimum:

- unit tests for product/pricing/inventory/cart/order/payment/fulfillment;
- integration tests for event publication and `rustok-index`;
- Postgres migration tests;
- contract tests for `/store/*` and `/admin/*`;
- contract tests cover all public use-cases;
- parity tests `REST <-> GraphQL`;
- router/OpenAPI smoke tests;
- tenant/RBAC regression tests;
- channel-aware regression tests after Phase 6 starts.

Release gates:

- Medusa-style transport cannot be considered stable without contract tests against live `/store/*` and `/admin/*`;
- checkout flow cannot be expanded without migration/integration coverage;
- provider-specific integration cannot be deployed before the provider SPI is stabilized;
- a separate sales-channel taxonomy cannot be created inside `commerce` while the platform-level `rustok-channel` remains the canonical channel layer;
- legacy compatibility surface cannot be pulled back for the convenience of local development.

## Update rules

1. When changing umbrella/runtime contract, update this file first.
2. When changing public surface, synchronize `crates/rustok-commerce/README.md` and `crates/rustok-commerce/docs/README.md`.
3. When changing the contract between `channel` and `commerce`, synchronize `rustok-channel` docs and central `docs/architecture/api.md`.
4. When changing module topology, transport contract or the `channel` vs `commerce` boundary, update `docs/index.md`, local docs of moved crates and ADR if necessary.
5. Any schema changes go through i18n audit: localized strings are not stored in base tables, display fields live only in `*_translations`.
6. Module-owned UI packages do not introduce package-local locale override: write-side uses the host-provided effective locale, and edit/detail hydration resolves translations by the same locale, with fallback only after an exact locale match attempt.
7. Read-side/runtime helpers do not compare locale by raw string: localized data resolution goes through shared locale normalization and a single fallback chain `requested -> tenant default -> first available`.


## Quality backlog

- [x] Update test coverage for key module scenarios.
- [x] Verify completeness and currency of `README.md` and local docs.
- [x] Lock/update verification gates for current module state.
