# rustok-pricing

## Purpose

`rustok-pricing` is the default pricing submodule of the `Ecommerce` family.

## Responsibilities

- Own the pricing service, price-related migrations, and pricing runtime metadata.
- Keep the decimal money contract working on the current runtime while preserving
  compatibility with legacy `prices.amount` and `compare_at_amount` storage.
- Provide a module-owned Leptos admin UI package in `admin/` for pricing visibility,
  sale markers, and currency-coverage inspection.
- Provide a module-owned Leptos storefront UI package in `storefront/` for
  public pricing discovery, currency coverage, and sale-marker visibility.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs, entities, and errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- `apps/admin` consumes `rustok-pricing-admin` through manifest-driven composition,
  while dedicated pricing write transport is still being split from the umbrella
  commerce surface.
- `apps/storefront` consumes `rustok-pricing-storefront` through manifest-driven
  composition for a public pricing atlas route.

## Entry points

- `PricingModule`
- `PricingService`
- `rustok-pricing-admin`
- `PricingView`

See also `docs/README.md`.
