# Crate API: rustok-commerce-storefront

## Public surface

### UI Components
- `CommerceView`: Root storefront checkout and aggregate commerce component.

### Core
- Route query shell state, checkout steps, and handoff models.

### Transport
- Dual-transport facade for checkout orchestration, shipping selection, and completion.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for `rustok-commerce`.
- Native Leptos server functions via `HostRuntimeContext` and GraphQL fallback.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-cart` for cart state snapshots and repricing on change.
- Collaborates with `rustok-pricing` for real-time tier and price list recalculation.
- Cooperates with `rustok-fulfillment` for shipping options and delivery selection.
- Coordinates with `rustok-payment-storefront` for payment collections.
- Collaborates with `rustok-order` for order finalization and transition.

## Minimum Contract Set

### Input DTOs/Commands
- `CommerceView`: Entrypoint storefront view mounting checkout workflow.
- Shipping method selection and checkout completion commands.

### Domain Invariants
- Checkout operations require valid cart identity and tenant isolation.
- Repricing triggered after delivery selection and before payment collection to avoid stale totals.
- Zero business logic inside UI view templates; flow orchestration lives in `core` and `transport`.

### Events / Outbox Side Effects
- Dispatches order creation, inventory reservation, and payment collection actions upon completion.

### Errors / Failure Codes
- Stale price and inventory exhaustion errors surfaced with clear user messaging.
- Network and server function errors mapped into user-visible alerts without UI panics.

