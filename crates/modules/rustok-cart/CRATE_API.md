# rustok-cart / CRATE_API

## Public Modules

`atomic_checkout_port`, `checkout_snapshot`, `dto`, `entities`, `error`, `guest_access`, `guest_access_http`, `marketplace_snapshot`, `migrations`, `module`, `ports`, `services`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `CartModule` implementing `RusToKModule` (slug `"cart"`) and `MigrationSource`.
- **Cart Management Services**:
  - `CartService`: Core shopping cart CRUD, adding/updating line items, shipping option selections, customer/guest assignment, and line item recalculation.
  - `CartMarketplaceSnapshotService`: Marketplace seller snapshots and multi-vendor delivery groupings.
- **Atomic Checkout Capabilities**:
  - `AtomicCartCheckoutPort`: Atomically binds and locks an active cart during checkout execution, preventing concurrent checkouts.
  - `in_process_atomic_cart_checkout_port` & `bind_in_process_atomic_cart_checkout`: In-process composition hooks for checkout runtimes.
- **Guest Access Utilities**:
  - `verify_guest_cart_token`, `verify_current_guest_cart_access`: Secure token verification for unauthenticated guest carts via headers or cookies.

## Domain Collaborations

- **`rustok-commerce`**: Acts as the primary orchestrator that transitions verified carts into staged checkout and completed orders.
- **`rustok-product`**: Source of truth for stable SKU/Product identity, publication, fulfillment and descriptive titles; not current sale-price authority.
- **`rustok-pricing`**: Re-pricing port provider for catalog and dynamic promotional discounts.
- **`rustok-fulfillment`**: Shipping option discovery and shipping method validation for cart delivery groups.
- **`rustok-tax`**: Tax rate calculation and cart tax line generation.
- **`rustok-region`**: Reads the base and country tax policy through `RegionService::read_tax_policy_in_tx` in the active Cart transaction, including the selected channel provider.

## Primary Public Types and Signatures

- `pub struct CartModule`
- `pub struct CartService`
- `pub struct CartMarketplaceSnapshotService`
- `pub trait CartCheckoutPort`
- `pub trait CartStorefrontPort`
- `pub trait CartPromotionPort`
- `pub trait AtomicCartCheckoutPort`
- `pub enum CartStatus`
- `pub struct CartResponse`
- `pub struct CartLineItemResponse`
- `pub struct CartLineItemPricingUpdate`: line UUID, quoted quantity, unit money and pricing adjustment.
- `pub struct CartContextUpdateRequest`
- `pub enum CartContextPricing`
- `pub struct CartContextPricingPlan`
- `pub struct CartPricingScope`
- `pub struct CartPricingContextGuard`: original priced or purpose-limited removal observation.
- `pub const MAX_PRICED_CART_LINES: usize = 1024`
- `pub enum CartError`
- `pub type CartResult<T>`

## Dependencies on Other RusToK Crates

- `rustok-core`
- `rustok-api`
- `rustok-region`
- `rustok-fulfillment`
- `rustok-tax`

## Minimum Contract Set

### Input DTOs/Commands

- `CreateCartInput`: Creates a new cart with optional currency, region, customer, and channel context.
- `AddCartLineItemInput` / `AddMarketplaceCartLineItemInput`: Adds a product variant with quantity and optional custom metadata.
- `UpdateCartContextInput`: Full replacement of email, region, country, locale and scalar shipping selection; `shipping_selections: None` keeps group selections except the single-group scalar patch, `Some([])` explicitly clears them. It does not change currency, customer or arbitrary metadata.
- `CartContextUpdateRequest`: One internal owner command with input, original `CartPricingContextGuard` and explicit `CartContextPricing`.
- `CartContextPricingPlan` / `CartPricingScope`: Complete prepared target-context prices and their original-line coverage; not browser-supplied price authority.
- `SetCartAdjustmentInput`: Applies promotion codes or manual discounts to cart totals.
- `CartShippingSelectionInput`: Attaches selected shipping options to deliverable line-item groups.

### Domain Invariants

- **Multi-Tenant Isolation**: Every cart operation is strictly partitioned by `tenant_id`; cross-tenant access fails closed with a validation or not-found error.
- **Status State Machine**: Active carts can be transitioned to completed or abandoned. Completed carts are immutable and cannot receive additional line items or modifications.
- **Guest Access Authorization**: Guest cart access requires a valid, cryptographic token presented in headers or cookies matching the cart's token hash.
- **Line Item Pricing Integrity**: Item prices, subtotals, tax lines, and discounts are computed server-side and never trusted from client inputs.
- **Atomic Checkout Exclusivity**: A cart under active checkout is locked; parallel checkout attempts are rejected with an atomic conflict error.

### Events / Outbox Side Effects

- CartService commits its Cart rows, line/adjustment/tax/shipping snapshots transactionally.
- A durable Cart-mutation receipt and Cart-owned mutation/outbox publication are not supplied by
  this service path; write-context key admission is not exactly-once/event durability evidence.
  Downstream event/projection integration remains an explicit owner-plan gate.

### Errors / Failure Codes

- `CartError` variants: `Validation`, `CartNotFound`, `CartLineItemNotFound`, `InvalidTransition`,
  `PricingContextChanged`, `Database`, `TaxBoundary`, `ShippingBoundary`.
- All errors preserve typed domain information and map consistently across HTTP and internal port boundaries.

## Priced mutation adoption (2026-10-30; source continuation)

- Cart owns the optimistic `CartPricingContextGuard` for an external quote/stock observation.
  It binds tenant/Cart identity, saved price/access context, update witnesses and the exact
  observed line identities, positive quantities and unit/total money. It is not a monotonic
  revision, authorization token, stock/price hold or durable command receipt.
- Priced capture/verification admit at most 1024 observed lines and reject malformed/duplicate/foreign
  entries. `capture_add` also refuses a new priced insertion at capacity. This is a priced-operation bound, not deletion, truncation or rewriting of historical
  Cart state. The Cart owner compares the observation under its existing row lock before writes.
- Explicit removal uses `capture_removal` and a target-only purpose, so oversized or invalid-price
  historical carts can still be reduced without a price/stock lookup. Removal observations cannot
  authorize price adoption, and decrement-to-remove still binds its original quantity.
- The canonical storefront provider must forward the guard for add, quoted quantity changes,
  repricing and removal; the guest boundary binds only already-authorized access evidence,
  never replacing the original content/context observation with a newer snapshot.
- Pricing updates carry the quoted per-line quantity. Bulk adoption rejects duplicates,
  unknown/deleted lines and quantity drift before any line/adjustment change. Observed/new line
  money uses canonical currency/fixed-point validation and checked quantity multiplication. It must not
  silently collapse or omit requested updates or scale an old tier quote to a newer quantity.
- Buyer quantity changes use Commerce's selected Pricing/Product/Inventory orchestration.
  Inventory sees checked aggregate same-SKU demand with the target line replaced, not added
  twice; Pricing still sees the target line's quantity. Missing SKU identity refuses rather
  than falling back to the stored unit price. Removal remains the explicit alternative.
- No historical migration, persisted SKU/state, external write capability, new retry loop or
  retry-identity policy is introduced. Source implementation is not compiled or runtime proof;
  the [owner plan](docs/implementation-plan.md) records remaining release gates.

## Atomic context and pricing adoption (2026-10-31; source continuation)

`CartContextUpdateRequest` is the single internal context command shared by storefront and
checkout callers. It carries full-replacement input normalized by the owner, the original material-input
observation and an explicit `CartContextPricing` disposition. Public buyer request shapes do not
accept money/plans from the browser.

- `Reprice` carries a complete `CartContextPricingPlan` bound to the target `CartPricingScope`.
  Commerce observes selected Pricing before the Cart transaction, with no stored-price or
  unavailable-capability fallback. Cart verifies original state, target scope and exact
  all-line UUID/quantity coverage before writes, then commits context, line/adjustment/
  marketplace, shipping/tax and totals in one existing Cart transaction.
- `Preserve` is an explicit owner checkout/materialization operation, not a fallback. A
  nonempty Cart may retain unit prices only when its currency, region, channel pair and locale
  price basis is unchanged. Country/shipping changes feed Cart-owned tax/shipping recalculation.
  Empty carts have no price basis to invalidate. Buyer storefront updates require `Reprice`.
- Context mutation requires active state. Checking-out/completed recovery does not reapply
  context or obtain new quotes. Checkout without a Pricing plan must not introduce another
  unit-price basis into a nonempty Cart; callers must prepare priced context first.
- The original guest/customer observation is retained; authorization binding never rebases
  it after owner IO. Failure/timeout before a write or any transactional failure leaves the
  previous committed Cart unchanged. Failure after commit still is not a durable outcome receipt.
- No cross-owner price hold, stock reservation, monotonic Cart revision, outbox/receipt protocol,
  historical migration or persisted-state rewrite is supplied by this source slice.

`CartResponse::project_context` uses the same shipping-selection interpreter as persistence,
including resolved-group overlap refusal and scalar clearing. It retains old money only as a
hypothetical input, clears old shipping availability and is never returned as a live acknowledgement.
`context_is_current` only compares a normalized target for already-prepared orchestration; it does
not authorize or commit anything. Snapshot preparation delegates to the atomic owner prepare;
already-frozen context commands refuse rather than silently ignoring a new input.

Commerce batches by expected Product parent and **per-line quantity**, deduplicates variants per
group, chunks at Pricing's 256-variant bound and correlates response count/order/SKU/parent/context
and explicit money presence. One inherited wait budget covers all price batches. Unsupported batch
capability is a refusal, not permission to switch providers, single reads or stored prices.
See the [source handoff](docs/implementation-plan.md#atomic-context-pricing-adoption-2026-10-31).
