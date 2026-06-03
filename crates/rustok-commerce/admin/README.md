# rustok-commerce-admin

Leptos admin UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce admin root view used by `apps/admin`.
- Acts as the commerce-owned shipping-profile registry plus cart-promotion operator surface while ecommerce UI ownership is split by module boundaries.
- Keeps the typed shipping-profile registry and aggregate cart-promotion orchestration inside the commerce package.
- Provides native `#[server]` transport helpers for operator-side cart promotion preview/apply over `CartService`, even while the package UI remains focused on shipping-profile ownership.
- Routes shipping-profile, promotion, and order-change operations through `admin/src/transport.rs`; default form policy lives in `admin/src/core.rs`, so the Leptos surface does not call raw `api::*` operations for covered flows.
- Publishes the aggregate post-order change operator surface for exchange/claim order changes created by the return decision tree, while lifecycle mutations still call the order-owned GraphQL service contract.
- Participates in the manifest-driven admin composition path through `rustok-module.toml`.
- No longer carries product CRUD; that catalog UI now lives in `rustok-product/admin`.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

- Keeps Leptos render/bind code in `admin/src/ui/leptos.rs`; `admin/src/lib.rs` only wires modules and re-exports `CommerceAdmin`.

## Entry Points

- `CommerceAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract plus native `#[server]` functions and shared auth hooks from `leptos-auth`.
- Coexists with `rustok-product-admin` and `rustok-fulfillment-admin` during the current UI split while other ecommerce admin slices still move to their module-owned packages.
- Consumes `shippingProfiles`, `shippingProfile`, `createShippingProfile`, `updateShippingProfile`, `deactivateShippingProfile`, and `reactivateShippingProfile`.
- Consumes `orderChanges`, `applyOrderChange`, and `cancelOrderChange` for post-order operator actions instead of duplicating exchange/claim rules in the UI package.
- Reuses `rustok-cart::CartService` for typed cart-promotion preview/apply transport with `orders:read` / `orders:update` permission gates instead of introducing a promotion-specific storage path.
- Should remain compatible with the host `/modules/{module_slug}` contract and generic shell.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
