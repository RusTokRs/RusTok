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

