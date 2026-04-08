# rustok-inventory

## Purpose

`rustok-inventory` is the default inventory submodule of the `Ecommerce` family.

## Responsibilities

- Own the inventory service, stock-level migrations, and normalized stock and reservation persistence.
- Keep `stock_locations`, `inventory_items`, `inventory_levels`, and `reservation_items`
  as the source of truth for ecommerce inventory runtime.
- Provide a module-owned Leptos admin UI package in `admin/` for inventory visibility,
  low-stock triage, and stock-health inspection.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs, entities, and errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- `apps/admin` consumes `rustok-inventory-admin` through manifest-driven composition,
  while dedicated inventory write transport is still being split from the umbrella
  commerce surface.

## Entry points

- `InventoryModule`
- `InventoryService`
- `rustok-inventory-admin`

See also `docs/README.md`.
