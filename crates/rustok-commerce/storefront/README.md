# rustok-commerce-storefront

Leptos storefront UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce storefront root view used by `apps/storefront`.
- Routes storefront checkout orchestration through `storefront/src/transport.rs`; route/query shell state lives in `storefront/src/core.rs` while native server functions and GraphQL fallback stay behind the transport facade.
- Keeps only aggregate storefront handoff UX that still spans multiple ecommerce modules.
- Participates in the manifest-driven storefront composition path through `rustok-module.toml`.
- Uses native Leptos `#[server]` calls to expose effective storefront context plus aggregate checkout workspace state from host request/tenant/channel wiring.
- Acts as the remaining storefront orchestration surface while read-side ownership already lives in split commerce modules.

- Keeps Leptos render/bind code in `storefront/src/ui/leptos.rs`; `storefront/src/lib.rs` only wires modules and re-exports `CommerceView`.

## Entry Points

- `CommerceView` - root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses host-provided locale plus native `#[server]` extraction of `RequestContext` and `TenantContext`.
- Remains the aggregate storefront hub while `rustok-region/storefront`, `rustok-product/storefront`, `rustok-pricing/storefront`, and `rustok-cart/storefront` own module-specific storefront surfaces.
- Owns the remaining checkout workspace for delivery-group shipping selection, `payment collection` reuse, and `complete checkout` actions over `?cart_id=`.
- Reprices cart line items after shipping-selection and checkout context updates so checkout pricing stays aligned with the pricing resolver.
- Reprices cart line items before `payment collection` creation and `complete checkout` to avoid stale pricing when price lists or quantity tiers are active.
- Carries typed checkout adjustment rows through the checkout workspace, including `scope` and
  sanitized metadata for cart/order snapshots, without owning promotion display labels or localized labels.
- Keeps checkout-context, delivery-selection, payment-collection, and other cross-domain orchestration concerns out of the host app.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.

## Documentation

- See [platform docs](../../../docs/index.md).
