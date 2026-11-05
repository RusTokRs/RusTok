# rustok-commerce

## Purpose

`rustok-commerce` is the `Ecommerce` umbrella/root module for RusToK's commerce family.

## Known Limitations / Pending Implementation

- The native Region owner-schema continuation is verified in Region and Cart.
  Commerce's older `pricing_service_test` target still imports private Pricing
  entities and has Product constructors missing `brands`. Its compilation and
  fixture execution remain blocked pending migration to the current owner APIs.
  See the [Region consumer record](docs/implementation-plan.md#region-consumer-verification-2026-11-05).

- The [Product/Inventory catalog deletion prerequisite](docs/implementation-plan.md#inventory-catalog-deletion-source-evidence)
  now preserves all referenced Inventory history and refuses individual nonzero counters through
  the existing native Product transaction. Finite HTTP/GraphQL policy and exact-effect rollback
  sources are synchronized; 31 Rust/10 Node definitions are uncompiled/unrun and I026 is
  unexecuted. This is not a general retirement workflow, a Leptos native CRUD cutover, an
  Inventory Index source/tombstone or Product step-8 certification.

- Inventory's anonymous quantity prerequisite now preserves exact original locations, excludes
  keyed allocations and refuses Cart/Order/line selectors instead of discarding them. Commerce's
  selected checkout still uses the durable identity port; do not route it through quantity recovery.
  The prerequisite's 27 Rust/10 Node definitions remain unrun. Reservation quantity-free facts,
  current context, acknowledged typed delivery and Search refetch are now separately source-wired
  by their owners; an additional 34 Rust/10 Node definitions are uncompiled/unrun. Canonical
  digest admission, actual SQL/relay/Iggy/restart/Search evidence, generic Index and step 8 remain
  open. Commerce/host is not a substitute publisher. See the
  [owner API](../rustok-inventory/CRATE_API.md#reservation-invalidation-facts-2026-11-01-source-only).

Return-restock identity now uses the selected price-free Order binding batch and Inventory's
existing native ledger, with source-only whole-batch/response/replay guards. Tests, Rust/SQL,
mounted/remote execution and historical shipment/cancel movement proof are still unverified.
Shipment/cancel/reopen now has a separate bounded source implementation: selected saved Order
bindings, tenant/Variant/original-allocation proof and an additive Inventory-owned native movement
proof. Missing historical provenance refuses; it is never reconstructed from cancelled status or
zero/deleted reservations. Compilation, regressions, SQL/carriers and mounted/remote execution
remain unverified. No full return lifecycle/outbox/Index or Product step-8 completion is claimed.
See the [shipment source handoff](docs/implementation-plan.md#shipment-stock-provenance-source-evidence).

Inventory adoption's new physical metadata projection, whole-batch rollback/contention and
same-identity recovery are source-only, uncompiled and unexecuted. No mounted/remote parity or
complete shipment/cancellation/return/outbox/Index convergence is certified. A metadata-capacity
refusal requires explicit reconciliation without discarding saved payloads; it is not a transient
retry policy. See the current continuation below; Product step 8 remains open.

## Current bounded checkout continuation

[Inventory-owned reservation adoption](docs/implementation-plan.md#inventory-owned-reservation-adoption-2026-10-31)
replaces direct Commerce mutations of Inventory owner rows with the already-selected identity
capability. It preserves Cart-/Order-line UUID lineage, allocation and exact replay, with separate
journal/checkpoint recovery. Unverified nonretryable outcomes park for reconciliation rather than
automatic compensation. Outer staged storefront DTOs are unchanged. This is uncompiled source with
unrun regressions, not SQL/runtime certification or completion of Product step 8.

## Responsibilities

- Provide `CommerceModule` metadata for the runtime registry.
- Serve as the umbrella entry point for the ecommerce family.
- Preserve the GraphQL surface while the Medusa-style REST transport expands.
- Expose the Medusa-style REST transport slice under `/store/*` and `/admin/*`.
- Resolve storefront cart line items from server-owned catalog/pricing data using `variant_id + quantity`, instead of trusting client-provided title and price.
- Orchestrate submodules of the ecommerce family through owner-crate services and typed ports, without masking owner APIs behind umbrella aliases.
- Own the checkout orchestration flow across cart, payment, order, and fulfillment submodules.
- Own store-context resolution across region, currency, and tenant locale policy.
- Apply channel-aware storefront availability on top of platform `ChannelContext` and `rustok-channel` bindings, without introducing a second sales-channel domain inside commerce.
- Apply shipping-profile compatibility between catalog products, storefront shipping discovery, cart context, and checkout validation, with typed product/variant bindings, typed line-item snapshots, and metadata normalization kept only as a backward-compatibility layer. An explicitly empty shipping-option `allowed_slugs` compatibility list represents an unrestricted option; malformed present compatibility metadata remains fail-closed.
- Expose first-class `shipping_profile_slug` on product and variant create/update/read contracts and `allowed_shipping_profile_slugs` on shipping-option contracts.
- Expose deliverability-aware cart and checkout contracts with `delivery_groups[]`, typed `shipping_selections[]`, `fulfillments[]`, and typed `fulfillment.items[]`, while keeping the old singular shipping/fulfillment fields only as single-group compatibility shims.
- Treat nullable `seller_id` as the canonical marketplace identity key across product, cart, order, checkout, and fulfillment contracts; `seller_scope` is not used as a runtime grouping or selection fallback.
- Expose admin/manual post-order fulfillment creation over REST and GraphQL with typed `items[]`, seller-aware delivery-group consistency checks, and remaining-quantity validation against order line items.
- Expose partial item-level `ship` / `deliver` adjustments over admin REST and GraphQL, with per-item shipped/delivered counters and a language-agnostic metadata-based audit trail.
- Expose explicit admin `reopen` / `reship` fulfillment recovery operations over REST and GraphQL, so post-order delivery corrections do not rely on implicit status rewrites.
- Expose admin return decision-tree transport over REST (`POST /admin/orders/{id}/returns/decision`) and GraphQL (`createOrderReturnDecision`) on top of `PostOrderOrchestrationService`, so `return_only` / `refund` / `exchange` orchestration stays service-owned.
- Keep the module-owned admin UI as an aggregate operator workspace for shipping profiles, cart promotions, and post-order order-change actions; exchange/claim apply/cancel actions call `orderChanges` / `applyOrderChange` / `cancelOrderChange` instead of embedding domain rules.
- Own the typed `shipping_profiles` registry and validate product/shipping-option references against active shipping profiles before write-path mutations are accepted.
- Resolve the effective shipping profile as `variant -> product -> default`, persist it into cart/order line-item snapshots, and use those snapshots instead of live product metadata for checkout deliverability decisions.
- Expose admin shipping-option management over REST and GraphQL (`list/show/create/update/deactivate/reactivate`) on top of `FulfillmentService`, so delivery compatibility and lifecycle are configurable without dropping to direct service calls.
- Expose admin shipping-profile management over REST and GraphQL (`list/show/create/update/deactivate/reactivate`) on top of `ShippingProfileService`.
- Expose only commerce-owned checkout/context/shipping-profile DTOs and shipping-profile entities; owner DTO/service contracts are imported directly from `rustok-cart`, `rustok-customer`, `rustok-product`, `rustok-region`, `rustok-pricing`, `rustok-inventory`, `rustok-order`, `rustok-payment`, and `rustok-fulfillment`. Region mappings remain private to Region. Its consumer fixtures use owner migrations and commands.
- Expose commerce-owned orchestration services such as `CheckoutService`, `StoreContextService`, `ShippingProfileService`, `PaymentOrchestrationService`, and `PostOrderOrchestrationService`.
- Keep shared/admin product, storefront product/catalog/order/cart/checkout, admin order/change/return, admin fulfillment, admin shipping, and admin payment HTTP handlers on narrow `CommerceHttpRuntime` state; remaining admin/storefront REST adapters are still cut over in separate owner-boundary slices.
- Keep commerce-owned orchestration code and leftover migrations not yet moved to new modules.
- Publish a module-owned Leptos admin UI package in `admin/` for host composition.
- Let the module-owned Leptos admin UI package keep the typed shipping-profile registry after product CRUD moved into `rustok-product/admin`, shipping-option UI moved into `rustok-fulfillment/admin`, order operations UI moved into `rustok-order/admin`, inventory visibility moved into `rustok-inventory/admin`, and pricing visibility moved into `rustok-pricing/admin`.
- Publish a module-owned Leptos storefront UI package in `storefront/` for host composition.
- Keep the aggregate storefront package focused on checkout orchestration while split modules publish their own storefront routes, with `rustok-region/storefront`, `rustok-product/storefront`, `rustok-pricing/storefront`, and `rustok-cart/storefront` already owning their public slices.
- Keep `rustok-commerce/storefront` focused on aggregate checkout workspace concerns such as delivery-group shipping selection, payment-collection reuse, and checkout completion over a selected cart.
- Publish the typed RBAC surface for commerce resources.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `rustok-commerce-foundation` for shared DTOs, entities, search helpers, and errors.
- Depends on `rustok-cart`, `rustok-customer`, `rustok-product`, `rustok-region`, `rustok-pricing`,
  `rustok-inventory`, `rustok-order`, `rustok-payment`, and `rustok-fulfillment` as the default cart,
  customer, product, region, pricing, inventory, order, payment, and fulfillment submodules of the ecommerce family.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Depends on `rustok-channel` for platform-level channel bindings and request-aware storefront visibility rules.
- Depends on `rustok-outbox` and `rustok-events` for transactional domain-event publishing.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- `apps/admin` consumes `rustok-commerce-admin` through manifest-driven `build.rs` code generation, with a module-owned commerce control room mounted under `/modules/commerce` for shipping-profile operations.
- `apps/admin` also consumes `rustok-fulfillment-admin` through the same manifest-driven composition path, with shipping-option CRUD and lifecycle now owned by the fulfillment module.
- `apps/admin` also consumes `rustok-order-admin` through the same manifest-driven composition path, with order list/detail/lifecycle now owned by the order module.
- `apps/admin` also consumes `rustok-inventory-admin` through the same manifest-driven composition path, with inventory visibility and stock-health inspection now owned by the inventory module.
- `apps/admin` also consumes `rustok-pricing-admin` through the same manifest-driven composition path, with pricing visibility and sale-marker inspection now owned by the pricing module.
- `apps/admin` also consumes `rustok-product-admin` through the same manifest-driven composition path, with catalog CRUD now owned by the product module instead of the aggregate commerce route.
- `apps/storefront` consumes `rustok-commerce-storefront` through manifest-driven `build.rs` code generation, with the aggregate checkout workspace mounted under `/modules/commerce`.
- `apps/storefront` also consumes `rustok-product-storefront` through the same manifest-driven composition path, with published catalog discovery now owned by the product module under `/modules/products`.
- `apps/storefront` also consumes `rustok-pricing-storefront` through the same manifest-driven composition path, with public pricing discovery now owned by the pricing module under `/modules/pricing`.
- `apps/storefront` also consumes `rustok-region-storefront` through the same manifest-driven composition path, with public region discovery mounted under `/modules/regions`.
- `apps/storefront` also consumes `rustok-cart-storefront` through the same manifest-driven composition path, with cart inspection and safe line-item decrement/remove actions mounted under `/modules/cart`.
- `rustok-module.toml` exports both surfaces through `[provides.admin_ui]` and `[provides.storefront_ui]`, so host wiring stays manifest-derived instead of relying on manual route registration.
- Declares permissions via `rustok-core::Permission` for `products`, `orders`, `customers`,
  `regions`, `payments`, `fulfillments`, `inventory`, and `discounts`.
- Transport adapters validate permissions against `AuthContext.permissions`, then invoke
  commerce services or direct tenant-scoped SeaORM reads where the module still owns the
  read-model assembly; post-order order-change preview/apply/cancel transport and
  return decision-tree transport stay backed by module services (`rustok-order::OrderService`
  and `PostOrderOrchestrationService`) rather than host-owned logic.
- Authenticated principals that lack a required Commerce Admin permission receive HTTP `403 Forbidden`; HTTP `401 Unauthorized` is reserved for missing authentication context, and protected Admin OpenAPI operations advertise both responses.
- Store guest-cart reads and mutations require the cart's server-issued guest capability; anonymous HTTP actor classification never substitutes for that capability, and cart-scoped Store operations advertise `403 Forbidden` when the capability is missing or invalid.
- Authenticated access to another customer's cart or order is an authorization denial (`403 Forbidden`); unauthenticated access to a customer-owned resource remains `401 Unauthorized`.
- Channel-aware price resolution is intentionally not part of the current storefront availability baseline and remains planned under Pricing 2.0.

## Capability classification gap

The current module graph declares Fulfillment as an unconditional Commerce dependency,
and current checkout assumes delivery groups for the shipping-aware path. That is
current executable truth, not the accepted target for all product kinds. Digital
products and digital order lines require no shipping capability; Commerce must remain
available for a digital-only flow when Fulfillment is absent.

The cutover must preserve a typed fulfillment requirement in product/cart/order
snapshots, form delivery groups only for physical lines, and require Fulfillment at
the affected operation boundary. Mixed carts must not assign a synthetic shipping
profile to digital lines. See
[`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

## Post-order lifecycle orchestration (2026-10-21, unverified)

- Admin order `cancel` is routed through `OrderCancellationOrchestrator` (decorator of
  `OrderAdminCommandPort`): it cancels live fulfillments, voids authorized collections, requests a
  full refund of captured money and only then cancels the order. Orders with shipped items are
  rejected (use a return). See `DECISIONS/2026-10-21-order-cancellation-orchestration.md`.
- Return refunds are valued on the server (`return_refund_value.rs`) from the order line price,
  discounts and tax; an explicit amount may only be lower. Physical lines cannot be returned beyond
  their shipped quantity (cumulative over live returns) when fulfillment records exist. See
  `DECISIONS/2026-10-21-return-refund-server-side-valuation.md`.
- Shipment moves stock in the fulfillment transaction (ship consumes, cancel of a shipped fulfillment
  restores, reopen of it reconsumes) and the operator can restock returned goods at return
  completion. See `DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md`.
- Order `shipped`/`delivered` follow fulfillment truth (`OrderShipmentProjector`, decorator of
  `FulfillmentAdminCommandPort`); manual order `ship`/`deliver` are refused while live fulfillments
  contradict them. See `DECISIONS/2026-10-21-order-shipment-status-projection.md`.

The targeted shipment source recheck also closes key/payload shortcuts, mutable-provider replay,
late-outcome preservation, nullable pending reopening and elapsed native budget gaps. The
[current handoff](docs/implementation-plan.md#targeted-shipment-correction-2026-11-01-not-an-audit)
records 58 focused Rust and 30 Node definitions, all uncompiled/unrun; it does not certify runtime.

### Known Limitations / Pending Implementation

- Product step-8 source adds aggregate UUID-based checkout preflight, exact quote/context and
  fulfillment checks, Inventory-owned scoped Cart add admission (not Product global counts),
  saved Channel resolution and scoped new identity allocation. See the
  [downstream plan](docs/implementation-plan.md#product-downstream-continuation-2026-10-30).
  This does not close step 8: remote Product identity for Pricing, Cart quantity/tier runtime certification,
  complete Order/Fulfillment/stock/event/outbox effects and selected mounted execution remain
  to verify. Direct `CheckoutService` is availability-only, not staged reservation durability.

- This code was written without a Rust toolchain: it has not been compiled, checked by native
  rustfmt or tested. Selected Scalar syntax/format checks and source guards are not Rust/runtime evidence.
  Run `cargo test -p rustok-commerce` (crate-scoped) before relying on it.
- Cancellation has no durable journal; it is a stateless re-runnable saga. The cancellation refund
  is created `pending` and is completed by the provider or operator.
- `complete_return` restocks only on operator decision (`metadata.restock = "all"` or
  `[{return_item_id, quantity}]`) and only through `ReturnCompletionOrchestrationService`; return
  completion paths that bypass it never touch stock. Restock goes to the item and location of the
  saved tenant/Variant allocation. Missing or ambiguous tracked reservations refuse; only an
  explicit SKU-less Order binding skips stock. The complete plan is owner-correlated before
  resolution effects and must survive completion unchanged; exact ledger replay compares saved
  identities and quantity. Fresh archived-location writes require explicit reconciliation,
  not a retryable storage loop; exact archived receipt replay remains read-only. Receiving
  locations lock before levels. No automatic reactivation/relocation or operator reactivation
  workflow is delivered. See the [source handoff](docs/implementation-plan.md#return-restock-identity-source-evidence).
- Return records can still be created on unpaid orders (a refund needs a captured collection).
- Shipment projection skips fulfillment changes made outside the admin command port (provider
  reconciliation) and never moves an order status backwards on reopen/reship.
- Stock moves on shipment through the fulfillment-owned `ShipmentStockPort`, composed here as
  `InventoryShipmentStockPort`; every ship-capable fulfillment service in this crate must be built
  with `fulfillment_service(db)` / `fulfillment_admin_command_runtime(db, registry)`. A bare
  `FulfillmentService::new` fails ship / cancel-shipped / reopen-cancelled on purpose. Migration
  `m20261021_000001` drops the former trigger and must run before the new binary takes traffic. See
  `DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md`. Provider reconciliation that
  applies results outside the fulfillment service does not move stock.

## Entry points

- `CommerceModule`
- `ShippingProfileService`
- `CheckoutService`
- `StoreContextService`
- `PaymentOrchestrationService`
- `PostOrderOrchestrationService`
- `graphql::CommerceQuery`
- `graphql::CommerceMutation`
- `controllers::routes`
- `admin::CommerceAdmin` (publishable Leptos package)
- `storefront::CommerceView` (publishable Leptos package)
- commerce-owned `dto` and `entities` modules; owner DTO/entity/service contracts are imported from owner crates directly

See also `docs/README.md`.

## Storefront Product HTTP policy (2026-10-19)

Mounted Product list/detail share a static presentation policy. **Forbidden now returns
403 and Conflict 409 instead of the prior generic 500**; missing/invisible Product
remains 404. Mapper diagnostics are bounded and owner-call correlations are opaque.

## Docs

- [CRATE_API](./CRATE_API.md)
- [Module docs](./docs/README.md)

### Cart quantity adoption source continuation

The shared Commerce workflow now performs quoted quantity changes for REST, GraphQL and native
with selected Product/Pricing and explicit Inventory ports. Stock demand replaces the target
quantity and includes siblings; price tiers remain per-line. Cart owns locked material-input
adoption and target-only removal. The unquoted buyer quantity write is retired. This is not a
monotonic Cart revision, stock/price reservation or durable receipt. The 2026-10-31 source
continuation below closes Cart-local context/pricing atomicity, not cross-owner execution.
Cart mutation outbox/receipt effects and downstream convergence remain pending; source regressions and runtime limitations are tracked in the [handoff](docs/implementation-plan.md#cart-quantity-adoption-2026-10-30).

### Atomic Cart context source continuation (2026-10-31)

A shared selected-Pricing preparer now produces complete target-context prices before the single
Cart-owned context command. REST/GraphQL/native shipping and native payment preparation consume
it; parent identity and per-line tiers survive deduplicated/chunked batches under one wait budget.
Explicit country/null and shipping clear semantics no longer fall back to the first regional
country or old selections. Checkout recovery preserves frozen context/prices and does not invoke
the context writer. Public buyer/cart DTO shapes remain unchanged.

Tests are not run; SQL/concurrency, mounted/native/browser/remote profiles and downstream/outbox
convergence remain open. See the [owner contract](../rustok-cart/CRATE_API.md#atomic-context-and-pricing-adoption-2026-10-31-source-continuation)
and [source handoff](docs/implementation-plan.md#atomic-cart-context-adoption-2026-10-31).

## Native physical stock outbox continuation

The [Inventory-owned invalidation contract](../rustok-inventory/CRATE_API.md#native-stock-outbox-2026-11-01-source-only)
now couples native stock changes with the canonical outbox. C still composes owners rather
than publishing substitute stock events. Preparation/exact replay/no movement emit none;
physical shipment with equal available counters still invalidates. The
[source handoff](docs/implementation-plan.md#native-stock-outbox-continuation-source-evidence)
adds 16 Rust/10 Node cases, all unrun. There is no new schema/capability, distributed atomicity,
relay/Index certification or Product step-8 completion.

## Canonical quantity writer prerequisite (2026-11-01; source only)

Native set/adjust and Product's real quantity-update hook now delegate to one Inventory-owned
bounded plan: current tenant/actor/SKU, saved positive identities, complete 4096 levels, original
location/current binding proof, checked signed arithmetic and exact effects. Quantity is global
available stock; only the existing active owner-default level's stocked counter changes.
Reservations/incoming/history/metadata/other locations are preserved. Missing state refuses;
no-op stock plans are read-only. Existing v1 stock notifications are preflighted and canonically
published on the same transaction; no negative snapshot is clamped or wire-broadened.

See the [source packet](../rustok-inventory/contracts/evidence/quantity-writes-source.json) and [owner contract](../rustok-inventory/CRATE_API.md#canonical-quantity-writes-2026-11-01-source-continuation).

### Known Limitations / Pending Implementation

33 Rust/10 Node focused definitions remain uncompiled/unrun; direct source checks are not SQL,
concurrency, client or runtime evidence. Native stock commands remain non-idempotent and current
Inventory client redaction is unchanged. No baseline, revision/tombstone, generic Index, full
Next/Leptos recovery parity or Product step-8 readiness is claimed.

## Inventory source-history continuation (2026-11-01; source only)

Native Product fresh bootstrap, quantity and exact guarded deletion now participate in the
[Inventory-owned baseline/revision journal](../rustok-inventory/CRATE_API.md#inventory-source-history-2026-11-01-source-continuation).
The bootstrap helper requires the actual caller transaction. A building baseline rejects actual
stock/identity effects, while stock no-op remains read-only; new source provenance failures use
conservative nonterminal storage/infrastructure envelopes, not terminal Product validation.
Existing Product receipt/rollback/publication stays authoritative; no Product revision substitutes
for an Inventory version and no Inventory-triggered Product refresh is added.

### Known Limitations / Pending Implementation

18 Rust/8 Node new source-history definitions are uncompiled/unrun. Baseline activation is an
explicit creator-actor-bound native Inventory API, with no operator HTTP/GraphQL/Next/Leptos
cutover, automatic reset or actor reconstruction. Versioned canonical publication/relay and
the selected consumer source continuation below supersedes the missing source/event/worker
code boundary; recovery/runtime gates remain open. No step-8 readiness is promoted.

## Selected Inventory Index consumer (2026-11-02; source only)

The [selected Distribution bridge](../rustok-distribution/docs/inventory-index.md) now authors
`inventory-stock-primary` / `rustok-inventory/stock`, real sealed Changed/activation listeners,
current authenticated native reads, durable generic persistence before Changed success and
original-baseline-UUID replay enqueue before activation success. The canonical Index runtime
registers bounded generic ModuleWork replay with exact job/checkpoint fences; no event actor,
baseline creator, receipt, journal or saved claim grants source authority. Old redelivery keeps
the currently loaded owner's immutable mutation UUID. Product/other remote consumers remain.
[Current source evidence](../rustok-inventory/contracts/evidence/source-index-consumer-source.json)
supersedes only the missing-consumer-code boundary of preceding source handoffs.

### Known Limitations / Pending Implementation

The runner still parks page failures terminally. Automatic failed-job backoff and authorized
original-job recovery remain unwired; crash/lease expiry and successful bounded yield resume
without UUID/checkpoint replacement. Lease attempts are not failure-only counters. Real credential/
policy/secret rotation, schema/digest/backend/concurrency/broker/restart/deployment evidence and
full native/HTTP/GraphQL/Next/Leptos operator recovery remain pending. No historical delivery is
fabricated; no live baseline/cutover before preflight/recovery. Tests/build/SQL/migrations/runtime
were not executed. Source/worker presence or initial enqueue is not Index or Product step-8 readiness.
