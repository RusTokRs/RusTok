# Split of `rustok-commerce` into `product`, `pricing`, and `inventory`

- Date: 2026-03-25
- Status: Accepted & Implemented

## Context

The exploratory migration plan for a Medusa-like model required ceasing to treat `rustok-commerce`
as a single module for catalog, pricing, and inventory. This contradicted the actual code structure, where:

- `CatalogService`, `PricingService`, and `InventoryService` lived in one crate;
- runtime wiring and `modules.toml` did not reflect separate platform modules;
- documentation simultaneously required a split and described `commerce` as a single optional module.

Without resolving this contradiction, it was impossible to honestly move forward to cart/order/customer/payment slices and to a
Medusa-compatible API surface.

## Decision

Accept and implement the first stage of the split directly in code:

- extract a common support crate `rustok-commerce-foundation` for shared DTO, entities, errors, and search helpers;
- extract `rustok-product` as a separate optional platform module for catalog, variants, options, and publishing;
- extract `rustok-pricing` as a separate optional platform module for the pricing slice;
- extract `rustok-inventory` as a separate optional platform module for the inventory slice;
- keep `rustok-commerce` as a transitional compatibility facade:
  - re-export shared contracts and extracted services;
  - maintain the legacy GraphQL/REST transport surface;
  - keep the order state machine and legacy migrations not yet extracted into separate modules.

The runtime manifest and server module registry should register `product`, `pricing`, `inventory`, and `commerce`
as separate optional modules, with `commerce` depending on `product`, `pricing`, `inventory`.

## Consequences

Positive:

- the contradiction between the exploratory plan and the actual module topology is resolved;
- platform/runtime wiring now reflects the real decomposition of the commerce domain;
- the next stage of the split can proceed from an honest base, not from a facade monolith.

Negative:

- `rustok-commerce` temporarily remains a transitional facade and still carries the transport/API surface;
- collections/categories/order-related parts are not yet extracted into separate modules;
- the inventory schema/model still requires further normalization according to the backlog migration plan.

Follow-up:

- bring `cart`, `order`, `customer`, `payment`, `fulfillment` to separate modules;
- extract remaining legacy migrations and transport surfaces from `rustok-commerce`;
- continue schema hardening and Medusa-compatible API contract tests.
