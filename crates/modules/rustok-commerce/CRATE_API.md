# rustok-commerce / CRATE_API

## Public modules

`controllers`, `dto`, `entities`, `error`, `graphql`, `services`, `state_machine`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `CommerceModule` implementing `RusToKModule` (slug `"commerce"`, dependencies `["tenant", "cart", "customer", "product", "region", "pricing", "inventory", "order", "payment", "fulfillment"]`) and `MigrationSource`.
- **Commerce Orchestration Services**:
  - `StoreContextService`: Resolves store locale, default region, and currency context across channel and tenant boundaries.
  - `ShippingProfileService`: Shipping methods, rates, delivery profiles, and zone mapping.
  - `StagedCheckoutService`: Journaled checkout with identity reservations, immutable order plans, order creation and payment stages. `CheckoutService` is the direct availability-check flow, not a durable reservation guarantee.
  - `PaymentOrchestrationService` & `PostOrderOrchestrationService`: Order payment settlement, post-order actions, and return/refund orchestration.
  - `OrderCancellationOrchestrator`: Compensating inventory releases and payment voids upon order cancellation.
  - `CheckoutCompensationSweepService`: Background sweeping of dangling checkout reservations and timed-out payment holds.
- **Event Listeners**:
  - `PaidOrderCreateLabelHandler`: Automatic fulfillment shipping label creation upon paid order events.
  - (Feature `marketplace-financial`) `MarketplacePaidOrderFinancialHandler`: Ingests paid order events into marketplace ledger allocation.

## Domain Collaborations

- **`rustok-cart`**: Active shopping carts, line items, and cart validation for checkout conversion.
- **`rustok-product` & `rustok-pricing`**: Product/SKU identity and authoritative context/tier pricing. Tax remains a separate owner contract.
- **`rustok-inventory`**: Identity reservations for staged checkout, public-channel availability, and fulfillment-owned shipment/return stock movement. Order placement alone is not a stock decrement.
- **`rustok-order`**: Order lifecycle state machine, order persistence, and line item tracking.
- **`rustok-payment`**: Payment transaction capture, authorization, and refund orchestration.
- **`rustok-fulfillment`**: Shipping provider integrations, fulfillment label generation, and tracking.
- **`rustok-customer`**: Customer profiles, saved addresses, and guest checkout resolution.
- **`rustok-outbox`**: Transactional outbox event publishing for order and checkout domain events.

## Primary public types and functions

- `pub struct CommerceModule`
- `pub struct StoreContextService`, `pub struct ShippingProfileService`, `pub struct CheckoutService`
- `pub struct PaymentOrchestrationService`, `pub struct PostOrderOrchestrationService`
- `pub struct graphql::CommerceQuery`, `pub struct graphql::CommerceMutation`
- `pub fn controllers::routes() -> Routes`
- `pub async fn read_storefront_cart_line_quote(...)`: shared Cart-to-Pricing orchestration
  adapter over a supplied `PricingReadPort`, admitted `PortContext`, saved `CartResponse`,
  original line UUID and requested positive quantity; returns Pricing's owner snapshot.
- `pub enum CommerceError`, `pub type CommerceResult<T>`

## Split boundary

- `rustok-commerce::dto` exposes only commerce-owned checkout/context/shipping-profile DTOs.
- `rustok-commerce::entities` exposes only commerce-owned shipping-profile entities.
- Owner service, DTO and entity contracts are imported directly from `rustok-product`, `rustok-region`,
  `rustok-pricing`, `rustok-inventory`, `rustok-cart`, `rustok-customer`, `rustok-order`,
  `rustok-payment` and `rustok-fulfillment`.
- `StoreContextService` remains in `rustok-commerce` as the umbrella policy layer that resolves
  region/currency/tenant locale context through owner ports.
- `graphql`, `controllers`, and `state_machine` remain in `rustok-commerce` under explicit module paths.
- `migrations()` exposes only umbrella-owned migrations that still remain in `rustok-commerce`.
  Product, pricing, and inventory migrations stay owned by their dedicated submodules.
- Product owns `variant_axes` and real sparse SKU assignments in its DTOs; attribute/option labels
  and translated titles are presentation, not a parallel option ontology or SKU identity.
- Product create/update/list/detail contracts now expose first-class `tags`; legacy
  `metadata.tags` is no longer part of the supported public contract.

## Downstream SKU checkout contract (2026-10-30; source only)

- Stable Variant UUID is the identity across owners. Missing/foreign/stale SKU membership,
  publication, saved channel or fulfillment/profile refuses; no first-SKU/text/tuple fallback.
- New checkout processing admits at most 1024 lines. Inventory demand is summed with checked
  i32 addition per UUID; original Cart/Order lines, prices, labels and recovery identities stay
  distinct. This is not a historical storage/retention limit.
- Quote observations must match SKU/parent, quantity, currency, region and saved Channel context.
  Explicit arbitrary Price List handoff is refused. Global source rows and automatic-list
  provenance may still satisfy the canonical request; source scope is not request scope.
- `StorefrontCartRuntime::new` requires Product, Pricing, guarded Cart and Inventory ports.
  REST/GraphQL/native supply the in-process Inventory owner explicitly. Cart add probes combined
  existing + new SKU demand in the canonical quote channel, correlates UUID/quantity and caps
  waiting at two seconds. Global Product stock fields and local backorder logic are not admission.
  This is an observation, not a stock hold; selected remote Inventory wiring is still separate.
- Saved Channel selectors are completed by the Channel owner without page/default substitution.
  Commerce caps its wait at two seconds. New identity reservations select visible active
  tenant-owned locations; exact receipt replay/release retain the original allocation.
- A nil guest actor delegates as the Commerce service, not as a User; customer/guest admission
  remains at the Cart/transport boundary. Internal call identity is not authorization.
- Shared line-quote admission binds the tenant/Cart/line, uses saved selectors only (including
  an explicitly global Cart), adopts saved locale or the admitted caller locale, and correlates
  every owner response before a price write. Actor/claims/roles/correlation/trace/key/deadline
  supplied by the caller are retained; no price selection/math is duplicated.
- Mounted staged checkout observes Pricing before acquiring the Cart write lock, then validates
  and applies that plan under the existing lock. Recovery adopts locked snapshots/journals,
  not current prices or a new SKU. Request-scoped Pricing captures the admitted Cart context.
  The direct CheckoutService remains availability-only; do not equate these durability models.
- Selected remote Pricing identity composition, all Order/Fulfillment/stock/outbox effects and
  mounted/concurrency execution remain open in the [implementation plan](docs/implementation-plan.md).
  No historical migration or stored state is rewritten by this source continuation.

## HTTP retry advice (2026-10-30; source only)

HTTP failures use the [canonical `rustok-web` envelope](../../libs/rustok-web/CRATE_API.md).
Typed-port mappers, nested OwnerPort and staged checkout preserve the already-known owner/final
runtime retry decision through `HttpError::with_retryable`; public code/message/status policies
remain unchanged. JSON false is retained; absent means unknown, never a status-derived default.
The DTO owns its optional non-nullable OpenAPI property. This is advice, not evidence of commit,
rejection, recovery completion, authorization or permission to issue another command/key.
Native server-function and GraphQL wire contracts are not changed by this HTTP addition.
See the [source handoff](docs/implementation-plan.md#canonical-http-retry-advice-2026-10-30) for
actual forwarding/client coverage and unexecuted verification gates.

## Events

- Publishes commerce domain events through the extracted services and outbox flow.
- Does not subscribe directly to external events in this crate.

## Dependencies on other RusToK crates

- `rustok-core`
- `rustok-api`
- `rustok-commerce-foundation`
- `rustok-product`
- `rustok-region`
- `rustok-pricing`
- `rustok-inventory`
- `rustok-events`
- `rustok-outbox`
- (dev) `rustok-test-utils`

## Common mistakes

- Re-introducing product, pricing, or inventory business logic back into `rustok-commerce` instead of the
  dedicated split module.
- Treating `rustok-commerce` as a low-level shared dependency of its own submodules. It is the umbrella/root
  module of the family, not the bottom layer.
- Changing order status outside the state machine.
- Bypassing `ValidateEvent` or the transactional outbox when publishing events.
- Moving transport adapters back into `apps/server` instead of extending
  `crates/modules/rustok-commerce/src/graphql/*` or `crates/modules/rustok-commerce/src/controllers/*`.

## Minimum Contract Set

### Input DTOs/Commands

- Public DTOs and command inputs are exported through this crate, even when implemented in
  `rustok-commerce-foundation`.
- Changes to public DTO fields are breaking changes and require synchronized updates in transport adapters.
- GraphQL and HTTP entry points remain part of the crate's public API.

### Domain Invariants

- Domain invariants remain enforced by services, DTO validation, and the order state machine.
- Multi-tenant boundaries, permission checks, and tenant-scoped queries remain mandatory.

### Events / Outbox Side Effects

- Domain events must keep using the transactional outbox flow.
- Event payloads and event types must remain backward compatible for downstream consumers.

### Errors / Failure Codes

- `CommerceError` and `CommerceResult<T>` define the public failure contract of the crate.
- Validation, auth, conflict, and not-found scenarios must preserve stable error semantics across
  HTTP, GraphQL, and internal callers.

## Cart quantity workflow (2026-10-30; source only)

`StorefrontCartRuntime::update_line_item_quantity` consumes the original admitted Cart read,
not a refreshed post-quote snapshot. The shared quote adapter supplies saved Cart scope and
saved-or-caller locale; selected Product verifies publication/parent/fulfillment identity, and
Inventory observes aggregate same-SKU **replacement** demand. Pricing still uses target-line
quantity. The guarded Cart port adopts the full observation under its row lock; missing SKU,
owner failure, mismatched data or stale material inputs refuse without a fallback write/retry.
REST/GraphQL/native public quantity request shapes stay unchanged. Explicit removal uses the
Cart-owned target-only observation, not whole-Cart priced limits. See the [handoff](docs/implementation-plan.md#cart-quantity-adoption-2026-10-30)
for source-only checks, unrun regressions and still-open receipts/context/outbox/runtime gates.


## Atomic Cart context preparation (2026-10-31; source only)

`prepare_storefront_cart_context(pricing, context, cart, input)` returns the single Cart-owned
`CartContextUpdateRequest`, without writing Cart. It takes the already-guarded original Cart,
captures its material fence before owner-normalized target projection and produces explicit
complete `Reprice` coverage. Public storefront inputs do not accept plan money or guards.

Expected Product parent plus **line quantity** determine groups; duplicate variants are quoted
once per group and fanned out to original line IDs, with chunks at Pricing's 256 bound. Context,
ordered UUID/parent/count and explicit price presence are correlated, and canonical fixed-point
money bounds precede adjustment projection. One inherited deadline bounds all price batches.
Unsupported capabilities, invalid identity/context and partial/unpriced responses refuse; no
stored-price, single-read, page-channel or local-provider fallback is used.

REST, GraphQL and native shipping issue one Cart context command after preparation/validation;
native payment preparation reuses it rather than retaining a duplicate price/adjustment builder.
Country/null and missing/group-null/scalar-null semantics are kept explicit. Checkout recovers
locked/completed state first and cannot introduce a new unit-price basis with planless Preserve.
Cart-local transaction ownership and runtime limitations are in the [handoff](docs/implementation-plan.md#atomic-cart-context-adoption-2026-10-31).


## Inventory reservation handoff (2026-10-31; source continuation)

`CheckoutOrderStageExecutor` shares its supplied `InventoryReservationIdentityPort` between
reservation allocation and `CheckoutOrderCreationExecutor` / `CheckoutInventoryOrderAdoptionService`.
Their constructors require the selected capability; adoption never resolves an embedded fallback.
`adopt_and_checkpoint` receives an admitted `PortContext`, renews the operation lease, validates
owner-returned Order/Cart-line provenance and correlates every saved reservation/Variant/quantity/
location/external identity with the complete Inventory batch reply before Commerce journal writes.

Inventory commits its binding first; Commerce commits mapping and `order_created` separately.
Post-owner failures retain same-identity recovery and bounded causes, not fictional rollback.
Nonretryable unverified outcomes park through `CheckoutOperationJournal::mark_reconciliation_required`;
its lease-fenced existing state/admission/park-event path is canonical. Retry advice alone is not
permission to compensate, replay automatically or rotate keys. Final staged storefront wrappers
preserve advice and expose existing unavailable/reconciliation codes without private causes.
Local pre-call replay read failures do not prove an earlier attempt had no effect; they receive
bounded recovery classification too. Recovery preserves the original typed adoption cause before
compensation lookup, while bounded journal messages never render raw local causes. The owner checks
full-batch physical metadata capacity before writes and refuses nonretryably without truncation.
Canonical missing/zero-deadline refusal precedes provider polling, not merely a zero-duration timeout.

Existing operational UUID provenance and known compaction histories are preserved; no schema,
historical migration, receipt, buyer DTO or client route changes. The
[source record](docs/implementation-plan.md#inventory-adoption-source-evidence) distinguishes static
verification from unrun tests and unproven Rust/SQL/contention/mounted/remote behavior. Full step-8
convergence remains open.

## Return-restock identity continuation (2026-10-31; source only)

`ReturnCompletionOrchestrationService` binds `metadata.restock` through its already-selected
`OrderReadPort` before refund/exchange/claim/completion effects. The mandatory price-free
saved-binding batch supplies stable Variant/quantity facts, with full tenant/Order/set and
quantity correlation. `PreparedReturnRestock` is an immutable in-memory plan projection,
not a receipt or another owner model. Completed-return parent/item facts must still match
before the existing native Inventory command runs.

The operator wire shape remains absent/null, `"all"`, or an array of exactly
`{return_item_id, quantity}`. Stock selections/required returned-item facts are bounded to 1024; IDs must be
non-nil and distinct, quantities positive i32 and within the returned/saved Order amount.
Only explicit SKU-less bindings can yield `untracked`; missing owner/Inventory data refuses.
The original completion request hash and return-item ledger keys remain mutation identities.
Positive owner call budgets are enforced with a 30-second ceiling. Complete Inventory outcome
correlation precedes the separate Commerce checkpoint. False owner retry advice parks the
same operation for reconciliation, never key rotation, relocation or automatic compensation.
Journal notes contain bounded static owner/code/kind/advice projections; the original typed
error remains available to the caller, without serializing arbitrary provider payloads.

Fresh stock cannot be written into an archived saved location under Inventory's existing
storage guard. Its bounded `inventory.return_restock_location_archived` cause parks the
same operation for reconciliation. Read-only archived receipt replay remains valid; no
warehouse reactivation, relocation, mutation-key change or new UI workflow is introduced.

REST, GraphQL and native callers retain the same Commerce service and public DTOs; no client
business-rule copy or provider fallback is introduced. No historical migration, receipt,
ledger or schema change exists. Tests, Rust compilation, SQL/locks, mounted/remote execution,
complete shipment/cancel/reopen provenance and stock outbox/Index convergence remain open.
See the [source handoff](docs/implementation-plan.md#return-restock-identity-source-evidence).

No-goods decisions do not acquire an Inventory/binding-read dependency or impose stock-batch
quotas on financial-only returns. Parent tenant/Order/return/completed-status scope remains
mandatory; a nonempty stock decision must still freeze and correlate all required item facts.

## Shipment/cancel/reopen provenance (2026-11-01; source only)

`InventoryShipmentStockPort` composes the already-selected `OrderReadPort` with Inventory's
native transaction-sharing commands. `inventory_shipment_stock_port(db, order_reads)`,
`fulfillment_service(db, order_reads)` and
`fulfillment_admin_command_runtime(db, registry, order_reads)` require the actual selected
Order reader. No peer-table lookup, fresh Catalog/Pricing eligibility, missing-binding SKU-less
fallback, warehouse relocation or unavailable-to-Noop conversion remains in this adapter.
The batch uses saved nullable Variant UUIDs and exact Order/line quantities, with complete
response correlation; maximum scope is 1024 items and the positive per-call budget is capped
at 30 seconds. Preparation runs before F's write transaction/carrier; apply rechecks the full
locked F snapshot and consumes the remaining original budget.

`JournaledFulfillmentOrchestrationService`, the lifecycle facade and reconciliation accept a
canonical `PortContext`, not a tenant-only reconstruction. Selected F commands win over native
composition. Local recovery delegates to `FulfillmentAdminCommandPort::retry_local_persistence`:
F alone reads its journal, restores the literal saved key (including deployed historical keys),
preserves current operator context, and commits native stock/progress/provider checkpoint
atomically without invoking a carrier. Committed recovery does not read O/I. Unsupported
selected recovery returns an actionable refusal, never native fallback.

Order cancellation requires paired selected F read/command capabilities, or both native. It
freezes the first total, requires full 100-parent pages and complete child projections, bounds
parents to 1024, validates tenant/Order/parent/child/line identities and counters, and calls
`cancel_pending_fulfillment` with a stable child key. The owner rechecks Pending under lock;
no generic full-cancel emulation is allowed. This is bounded planning, not a cross-page snapshot
or distributed transaction.

The five GraphQL lifecycle fields require a caller-owned `idempotencyKey` of 1..=191 UTF-8 bytes:
`shipFulfillment`, `deliverFulfillment`, `reopenFulfillment`, `reshipFulfillment`, `cancelFulfillment`.
Use a new key for a distinct intent and retain the same key/payload for uncertain recovery.
`createFulfillment` keeps its separate input-derived create identity. HTTP/GraphQL preserve
`AuthContext::port_actor()` (human versus OAuth service), permissions/scopes, channel, locale
and actual request correlation; GraphQL also verifies target/Auth/Tenant scope. AuthContext
has no roles field. Native contexts retain their own roles/tracing/causation unchanged.

Native movement and F state share one transaction, not O reads, the carrier or Commerce/Order
projection. Header-only fresh Ship is unknown scope: native composition refuses, Unconfigured
fails before carrier, and only an explicitly selected Noop may opt out. Pending cancellation,
delivery and undo-delivery have no stock dependency. The additive proof migration is unexecuted;
no historical backfill, return-ledger PK change or applied migration rewrite occurs.
The rollout also includes F's forward-only `m20261101_000121_enforce_provider_effect_admission`
after F120: no automatic ERROR reclaim, explicit same-key no-effect Pending requeue and retained
success-reference CAS. Both new migrations are unexecuted; drain old stock/provider writers
before coordinated installation/enabling. Local recovery/native wrappers consume the original
remaining budget across journal/replay reads, not a refreshed relative deadline.
Retained F literal keys pin original provider selection before mutable options; ambiguity and
changed replay payloads refuse. Late outcomes are monotonic and quarantined success requires
explicit owner reconciliation. HTTP defensively verifies Tenant/Auth/Request agreement and
GraphQL rejects contradictory supplied RequestContext; the canonical HTTP extractor already
checks that coherence. Native lock/read time and final stock/progress/receipt commits consume
the same budget. Pending-cancel reopening uses actual nullable timestamp/progress facts.

See the [owning amendment](../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#shipment-identity-and-native-movement-provenance-amendment-2026-11-01)
and [source evidence](contracts/evidence/shipment-stock-provenance-source.json).
Tests/compilation, PostgreSQL/SQLite contention and rollback, carriers, mounted/remote execution,
unpaid-return eligibility, partial-inspection top-ups and full event/outbox/Index convergence
remain open; Product step 8 is not complete.

## Native stock invalidation continuation (2026-11-01; source only)

C keeps selected shipment/returned-stock composition and immutable saved context/facts; native
I, not C or F, publishes the existing root `InventoryUpdated` in the physical write transaction.
The [Inventory-owned contract](../rustok-inventory/CRATE_API.md#native-stock-outbox-2026-11-01-source-only)
defines global descriptive quantities, physical-change invalidation, strict ownership/bounds,
current actor and no-publication replay/preparation semantics. An outbox failure rolls back
local stock/proof/receipt/counters, not carrier/financial/Order/C effects. No transport/SDK/event
schema change is made, so HTTP/GraphQL/native continue through the same owner composition.
The [source handoff](docs/implementation-plan.md#native-stock-outbox-continuation-source-evidence)
adds 16 Rust/10 Node definitions, all unrun; Search wiring is not generic Index/runtime proof.

## Anonymous quantity prerequisite (2026-11-01; source only)

Inventory's native quantity release no longer drains arbitrary/identity reservation rows or
independent warehouse counters. It only releases proved anonymous allocations at their original
levels and protects every live claim. Scoped quantity requests refuse with
`inventory.reservation_identity_required`; selected Commerce checkout/adoption/shipment contracts
remain identity-based. A key/retry hint does not make quantity commands replay-safe.

See the [Inventory contract](../rustok-inventory/CRATE_API.md#anonymous-quantity-reservations-2026-11-01-source-only).
The registered `inventory_service_test::quantity_release` definitions remain uncompiled/unrun
alongside the owner pure cases (27 Rust total, 10 Node definitions). No reservation notification
publisher or Index/runtime proof is added; prior physical-stock evidence remains separate.
