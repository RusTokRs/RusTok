# rustok-cart-storefront

Leptos storefront UI package for the `rustok-cart` module.

## Responsibilities

- Exposes the module-owned storefront cart route used by `apps/storefront`.
- Shows cart read-side state, line items, and delivery-group snapshots from the cart boundary.
- Shows typed adjustment totals, language-neutral source identity, adjustment `scope`, and sanitized
  metadata from the cart boundary instead of collapsing everything into summary-only counters.
- Supports safe cart-owned line-item decrement and remove actions without taking over checkout orchestration,
  while repricing line items through the pricing resolver on quantity change.
- Uses native Leptos `#[server]` calls as the default internal data layer and keeps GraphQL as fallback.
- Keeps cart UI policy and display/view-model mapping in a framework-agnostic `core/` layer and keeps Leptos rendering in `ui/leptos.rs`, routing actions through a thin `transport` facade.
- Leaves checkout completion and broader cross-domain orchestration inside `rustok-commerce`.

## Entry Points

- `CartView` - root storefront view re-exported from `ui::leptos` and rendered from the host storefront slot registry.
- `core::view_model` helpers re-exported from `core` - framework-agnostic display mapping for cart summary, adjustments, delivery groups, and line items.
- `transport::fetch_cart` - thin UI-facing facade for native-first cart reads with GraphQL fallback, while preserving core validation errors without retry.
- `transport::decrement_line_item` - thin UI-facing facade for safe line-item decrement with the same validation-vs-fallback policy.
- `transport::remove_line_item` - thin UI-facing facade for safe line-item removal with the same validation-vs-fallback policy.
- `transport/{native_server_adapter,graphql_adapter}.rs` internals - native `#[server]` and GraphQL fallback adapter calls behind the facade.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Reads `CartService` through server functions and enforces customer-owned cart access with the host auth context.
- Stays compatible with locale-prefixed module routes via `UiRouteContext::module_route_base()`.
- Coexists with the `rustok-commerce` storefront/transport layer while checkout and shipping orchestration remain aggregate concerns.

## Documentation

- See [platform docs](../../../docs/index.md).
