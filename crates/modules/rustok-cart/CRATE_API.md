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
- **`rustok-product`**: Source of truth for product variants, titles, and unit base pricing for cart items.
- **`rustok-pricing`**: Re-pricing port provider for catalog and dynamic promotional discounts.
- **`rustok-fulfillment`**: Shipping option discovery and shipping method validation for cart delivery groups.
- **`rustok-tax`**: Tax rate calculation and cart tax line generation.

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
- `pub enum CartError`
- `pub type CartResult<T>`

## Dependencies on Other RusToK Crates

- `rustok-core`
- `rustok-api`
- `rustok-commerce-foundation`
- `rustok-fulfillment`
- `rustok-tax`

## Minimum Contract Set

### Input DTOs/Commands

- `CreateCartInput`: Creates a new cart with optional currency, region, customer, and channel context.
- `AddCartLineItemInput` / `AddMarketplaceCartLineItemInput`: Adds a product variant with quantity and optional custom metadata.
- `UpdateCartContextInput`: Modifies region, currency, customer, or metadata bindings for an existing cart.
- `SetCartAdjustmentInput`: Applies promotion codes or manual discounts to cart totals.
- `CartShippingSelectionInput`: Attaches selected shipping options to deliverable line-item groups.

### Domain Invariants

- **Multi-Tenant Isolation**: Every cart operation is strictly partitioned by `tenant_id`; cross-tenant access fails closed with a validation or not-found error.
- **Status State Machine**: Active carts can be transitioned to completed or abandoned. Completed carts are immutable and cannot receive additional line items or modifications.
- **Guest Access Authorization**: Guest cart access requires a valid, cryptographic token presented in headers or cookies matching the cart's token hash.
- **Line Item Pricing Integrity**: Item prices, subtotals, tax lines, and discounts are computed server-side and never trusted from client inputs.
- **Atomic Checkout Exclusivity**: A cart under active checkout is locked; parallel checkout attempts are rejected with an atomic conflict error.

### Events / Outbox Side Effects

- Cart mutations, completions, and abandonments publish domain events through the transactional outbox flow.
- Event payloads adhere to platform schema contracts and remain backward compatible.

### Errors / Failure Codes

- `CartError` variants: `NotFound`, `Validation`, `Conflict`, `Forbidden`, `Database`, `Internal`.
- All errors preserve typed domain information and map consistently across HTTP and internal port boundaries.
