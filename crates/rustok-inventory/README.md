# rustok-inventory

## Purpose

`rustok-inventory` is the default inventory submodule of the `Ecommerce` family.

## Responsibilities

- Inventory service, stock-level migrations, and inventory runtime metadata.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs/entities/errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.

## Entry points

- `InventoryModule`
- `InventoryService`

See also `docs/README.md`.
