# `rustok-region`

`rustok-region` is the default region submodule in the `ecommerce` family.

## Purpose

- own commerce regions as a first-class domain module;
- keep currency, countries, and tax policy close to the region boundary;
- keep tax provider selection as a typed `tax_provider_id` field on the region
  contract instead of hiding it in metadata;
- provide the default region catalog used by the `commerce` umbrella.

## Responsibilities

- manage the `regions` schema;
- create, update, fetch, and list tenant regions;
- resolve a region by country for storefront and checkout policy.
- own the typed `tax_provider_id` write/read contract for tax policy routing.
- expose a module-owned admin route for region CRUD.
- expose a module-owned storefront route for public region discovery.
- keep region admin native transport on `rustok_api::HostRuntimeContext`, not Loco `AppContext`.

## Interactions

- `rustok-commerce` uses `rustok-region` as the umbrella/root policy input for locale and currency resolution;
- `rustok-tax` reads the typed region tax policy snapshot, including
  `tax_provider_id`, through commerce/cart runtime;
- `rustok-pricing` may reference `region_id` in prices but does not own region lifecycle;
- platform tenant locale tables remain platform-owned and are only read by the umbrella policy layer.

## Entry points

- `rustok_region::RegionModule`
- `rustok_region::RegionService`
- `rustok_region_admin::RegionAdmin`
- `rustok_region_storefront::RegionView`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
