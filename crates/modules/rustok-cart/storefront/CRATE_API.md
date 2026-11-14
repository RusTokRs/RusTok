# RusToK Cart Storefront API Specification

## Public surface

### Public modules
- `core`: Framework-agnostic display mapping, view-models, and cart calculations.
- `model`: Storefront data models for cart representations and line items.
- `transport`: Thin UI-facing transport facade and adapters (native server and GraphQL).
- The private `ui` layer binds the Cart controller to Leptos views and browser events.

### Re-exports
- `CartCheckoutHandoffCard`, `CartClient`, `CartHeaderTrigger`, `CartView`
- `BuyerCartController`, `CartCapability`, `use_buyer_cart`

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for the `rustok-cart` domain.
- Compatible with Leptos hydration and SSR modes via `rustok-ui-core`, `rustok-ui-i18n`, and `rustok-ui-transport`.

## Domain Collaborations
- Collaborates with `rustok-cart` domain service for customer-owned cart operations.
- Interacts with `rustok-pricing` for dynamic line-item recalculation and currency formatting.
- Coordinates checkout handoff with `rustok-commerce`.

## Minimum Contract Set

### Input DTOs/Commands
- `CartView`: Entrypoint route component for full storefront cart display.
- `CartClient`: Host-configured Cart controller, modal drawer and landing event adapter. Its selection callback returns the effective merged URL, or `None` on refusal; a verified Product quote context does not require Cart to be enabled.
- `BuyerCartController`: Same-owner operation reservation spans writes and synchronous recovery; abandoning an operation leaves the owner unavailable, not proof of server cancellation.
- `decrement_line_item`: Transport command reducing item quantity safely.
- `remove_line_item`: Transport command removing item from active cart.

### Domain Invariants
- Cart modifications must preserve customer and channel isolation.
- Zero business logic in UI view layers; all mapping resides in `core`.
- Line-item adjustments preserve precision and validate quantities non-negatively.

### Events / Outbox Side Effects
- Emits line-item change requests and cart mutation commands dispatched to backend transports.

### Errors / Failure Codes
- Validation failures propagate without masking or unhandled panics.
- Transport errors map to user-visible feedback without crashing UI view components.


## Quantity/removal adoption (2026-10-30; source only)

Native decrement/remove use the guarded Cart provider and preserve the original read through
Cart's locked adoption. Quoted quantity changes share Commerce's Product/Pricing/Inventory
workflow with REST/GraphQL; native price/discount mapping and raw CartService mutation are not
alternate paths. A target-only removal observation cannot authorize price writes and does not
strand oversized/invalid-price historical carts. Actual caller scope/claims are retained; guest
access is validated by the owner before projection sanitization. The endpoint/input/cart DTO
contract is unchanged. Owner failures use the shared static inspect-before-retry envelope.
Source regressions are unrun; receipts/exact replay, context-patch atomicity, native/browser/
remote/SQL/concurrency and authorization-revocation evidence remain open in the [owner plan](../docs/implementation-plan.md#priced-quantity-adoption-2026-10-30).
