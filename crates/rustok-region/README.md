# `rustok-region`

`rustok-region` is the default region submodule in the `ecommerce` family.

## Purpose

- own commerce regions as a first-class domain module;
- keep currency, countries, and tax policy close to the region boundary;
- provide the default region catalog used by the `commerce` umbrella.

## Responsibilities

- manage the `regions` schema;
- create, update, fetch, and list tenant regions;
- resolve a region by country for storefront and checkout policy.

## Interactions

- `rustok-commerce` uses `rustok-region` as the umbrella/root policy input for locale and currency resolution;
- `rustok-pricing` may reference `region_id` in prices but does not own region lifecycle;
- platform tenant locale tables remain platform-owned and are only read by the umbrella policy layer.

## Entry points

- `rustok_region::RegionModule`
- `rustok_region::RegionService`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
