# `rustok-region`

`rustok-region` is the default region submodule in the `ecommerce` family.

See [CRATE_API.md](CRATE_API.md) for the verified public crate API specification and platform capability bindings.

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
- keep region admin native transport on `rustok_api::HostRuntimeContext`, not a host-wide `AppContext`.

## Interactions

- `rustok-commerce` uses `rustok-region` as the umbrella/root policy input for locale and currency resolution;
- `rustok-tax` reads the typed region tax policy snapshot, including
  `tax_provider_id`, through commerce/cart runtime;
- `rustok-pricing` validates quote region ownership and currency through `RegionReadPort`; it does not own region lifecycle or compute region taxes;
- native Cart reads `RegionTaxPolicy` through `RegionService::read_tax_policy_in_tx`
  in its current write transaction. Region owns the private mappings and reads
  the base policy and country rules in one statement;
- platform tenant locale tables remain platform-owned and are only read by the umbrella policy layer.

## Known Limitations / Pending Implementation

The create/read and Cart tax-policy slices have scoped SQLite and PostgreSQL
coverage. The complete module remains a scaffold reference candidate. Backend
root-file placement, UI test slots, the oversized admin core, and Next.js
manifest bindings still fail the architecture lint. Mounted client parity and
the full `RegionReadPort` failure-envelope matrix remain unverified. The
[owner plan](docs/implementation-plan.md#scaffold-reference-candidate) tracks
these gates.

The transactional tax-policy entrypoint requires native composition sharing
the Cart database. A separately deployed provider needs an explicit snapshot
adoption contract before replacing that entrypoint.

## Entry points

- `rustok_region::RegionModule`
- `rustok_region::RegionService`
- `rustok_region_admin::RegionAdmin`
- `rustok_region_storefront::RegionView`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)
