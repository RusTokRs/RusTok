# rustok-commerce-storefront

Leptos storefront UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce storefront root view used by `apps/storefront`.
- Keeps only aggregate storefront handoff UX that still spans multiple ecommerce modules.
- Participates in the manifest-driven storefront composition path through `rustok-module.toml`.
- Uses native Leptos `#[server]` calls to expose effective storefront context from host request/tenant/channel wiring.
- Acts as the remaining storefront orchestration surface while read-side ownership already lives in split commerce modules.

## Entry Points

- `CommerceView` - root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses host-provided locale plus native `#[server]` extraction of `RequestContext` and `TenantContext`.
- Remains the aggregate storefront hub while `rustok-region/storefront`, `rustok-product/storefront`, and `rustok-pricing/storefront` own public discovery/read surfaces.
- Keeps checkout-context, delivery-selection, payment-collection, and other cross-domain orchestration concerns out of the host app.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.

## Documentation

- See [platform docs](../../../docs/index.md).
