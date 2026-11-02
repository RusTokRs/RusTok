# RusToK Cart Storefront API Specification

## Public surface

### Public modules
- `core`: Framework-agnostic display mapping, view-models, and cart calculations.
- `model`: Storefront data models for cart representations and line items.
- `transport`: Thin UI-facing transport facade and adapters (native server and GraphQL).
- `ui`: Storefront Leptos views and interactive cart components.

### Re-exports
- `CartCheckoutHandoffCard`, `CartDrawer`, `CartDrawerState`, `CartFloatingTrigger`, `CartHeaderTrigger`, `CartView`, `use_cart_drawer`

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
- `CartDrawer`: Interactive slide-out drawer presenting cart contents.
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
