# rustok-cart

## Purpose

`rustok-cart` is the default cart submodule of the `Ecommerce` family.

## Responsibilities

- Own the cart write-side schema and line item lifecycle.
- Persist the storefront cart context snapshot across region, locale, customer, and selected shipping intent.
- Persist typed cart adjustments as language-neutral promotion/discount snapshots.
- Persist first-class `shipping_total` so selected shipping options contribute to
  cart totals instead of remaining an implicit checkout-only side input.
- Delegate tax calculation through `rustok-tax::TaxCalculationPort` and snapshot typed tax-line
  `provider_id` instead of hardcoding region tax math directly inside the cart
  module.
- Keep cart snapshots independent from catalog ownership.
- Support repricing line items via the pricing resolver when quantity or
  storefront context changes, normalizing discounted items into
  `base/compare-at unit_price + pricing-owned adjustments` so persisted unit
  prices do not drift across channel/region boundaries.
- Prepare a stable handoff point for later checkout and order placement flows.
- Publish an owner-managed in-process `CartCheckoutPort` factory for explicit
  checkout and cart-read consumers.
- Publish an owner-managed `CartStorefrontPort` for storefront cart reads,
  mutations, and repricing without exposing `CartService` to consumers.
- Publish an owner-managed `CartPromotionPort` for cart and shipping promotion
  previews and writes without exposing promotion service methods to consumers.
- Publish a module-owned storefront cart workspace for cart inspection.
- Own the Axum guest-cart capability middleware used by HTTP hosts and owner
  transport tests.
- Require the guest-cart capability for guest carts even when a downstream port call uses a trusted Service/System actor; trusted actor kinds do not substitute for the guest token.

## Interactions

- Depends on `rustok-core` for module contracts.
- Used by `rustok-commerce` as the default cart submodule of the ecommerce family.
- Keeps locale and country snapshots normalized, while letting `rustok-commerce` own cross-module
  store-context resolution against tenant locale policy and regions.
- Keeps adjustment source identity in `source_type/source_id` and never stores promotion display labels
  as cart-owned business data.
- Treats `cart_adjustments` as the discount source of truth, while pricing-driven
  repricing rewrites only pricing-owned adjustments and preserves non-pricing ones.
- Computes totals as `subtotal - adjustments + shipping_total (+ tax when the
  region is tax-exclusive)`, giving checkout and payment collection a stable
  total contract before shipping promotions are introduced.
- Keeps tax provider identity explicit in cart tax-line snapshots, so future
  external tax engines can replace the default runtime without breaking the
  downstream cart/order contract.
- Reads native Region tax policy through the Region owner service in the Cart
  write transaction, preserving rollback of line and tax snapshot changes.
- Carts without a selected Region recalculate without accessing Region storage.
  A configured missing or foreign Region refuses the mutation atomically.
- Persists discounted add-to-cart snapshots atomically, so a storefront line item is
  not created in a temporary “sale-only unit price” state before pricing adjustments land.
- Exposes a typed promotion runtime for cart-level and line-item percentage/fixed
  discounts, so callers no longer need to rely on raw full-replace adjustment writes.
- Extends that typed promotion runtime to shipping scope, keeping shipping
  discounts as explicit adjustments instead of mutating `shipping_total`.
- Keeps product and variant references as snapshots so the cart domain does not depend on
  the product module as a lower-level shared layer.
- `apps/storefront` mounts `rustok-cart/storefront` via manifest-driven composition.

## Fulfillment capability gap

Cart stores a typed per-line fulfillment requirement and builds delivery groups from the
physical seller/profile snapshots only. Digital lines participate in price/payment/order
totals without a synthetic shipping group. The quantity workflow verifies the retained kind
and effective profile against Product; it does not recatalog line history. The remaining gap
is runtime/profile evidence: missing Fulfillment must block only affected physical operations,
not Cart or a digital-only checkout, and mixed/native/headless/remote behavior remains unverified. See
[`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

## Entry points

- `CartModule`
- `CartService`
- `dto::*`
- `entities::*`
- `CartView`

## Docs

- [CRATE_API](./CRATE_API.md)
- [Module docs](./docs/README.md)


## Known Limitations / Pending Implementation

The Region tax-policy read requires the native shared-database profile.
Separately deployed Region providers need an explicit snapshot adoption
contract. Scoped tests cover the local policy and rollback boundary. Mounted
client and remote-provider parity remain open.

The 2026-10-30 source continuation implements bounded Cart-material-input adoption, not a
monotonic whole-Cart revision or durable command receipt. Priced add/change/reprice operations
capture the exact tenant/Cart, saved price/access scope and line identity/quantity/money/
fulfillment observations, then compare under Cart's existing row lock. A current guest admission
binds only authorized capability evidence, never a refreshed content snapshot. Priced processing
is bounded to 1024 observed lines; insertion at capacity refuses. Read history is not truncated,
and explicit removal uses a target-only observation so larger/corrupt-price carts are not stranded.

Public storefront quantity writes cannot scale a saved price without a fresh owner quote.
Commerce checks per-line tier quantity and aggregate replacement stock demand through selected
owners; native, REST and GraphQL use the same workflow. Missing SKU identity refuses and explicit
removal/reconciliation remains available. Bulk duplicate/unknown/stale updates refuse atomically.
Both Marketplace Cart snapshot mutations also take that Cart lock before their reads/writes.
Root/module factories expose one admitted storefront provider, not a raw CartService port bypass.
The existing trusted CartService snapshot-materialization helpers are not public buyer transports
or proof of Product/Pricing/stock eligibility.

Price/stock reads still precede the Cart lock: this is not a cross-owner price/stock hold.
Context patch and complete prepared repricing now commit in one Cart-owned transaction; selected
Pricing observes the normalized target before the lock. A nonempty Cart cannot preserve prices
across a changed unit-price basis; checkout recovery never reapplies context or quotes.
Per-dispatch native keys and local recovery hints are not durable receipts; unknown outcomes must not create automatic retries/keys. CartService
has no Cart-mutation outbox/receipt publication in this path; downstream convergence remains open.
Remote Inventory/provider execution, mounted auth revocation, database/locking/concurrency and
native/browser/runtime verification remain unvalidated. New regression sources are unrun; see the
[owner plan](docs/implementation-plan.md#priced-quantity-adoption-2026-10-30).


### Atomic context source continuation (2026-10-31)

REST, GraphQL and native buyer context writes share `CartContextUpdateRequest` with original
observation and a complete target price plan. Country choice/null, group clear and single-group
scalar clear remain explicit; overlapping resolved groups refuse. Snapshot preparation delegates
to atomic owner preparation, while locked/completed recovery only reads its frozen state.
This is Cart-local database atomicity, not cross-owner IO atomicity or durable outcome evidence.
New regression sources are unrun; see the [handoff](docs/implementation-plan.md#atomic-context-pricing-adoption-2026-10-31).
