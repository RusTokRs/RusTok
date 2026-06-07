# rustok-inventory

## Purpose

`rustok-inventory` is the default inventory submodule of the `Ecommerce` family.

## Responsibilities

- Own the inventory service, backend admin read-side service, native admin stock write helpers, stock-level migrations, and normalized stock and reservation persistence.
- Keep `stock_locations`, `inventory_items`, `inventory_levels`, and `reservation_items`
  as the source of truth for ecommerce inventory runtime.
- Provide `AdminInventoryReadService` as the inventory-owned backend read model for admin
  product, variant, price, stock, and translation visibility; variant availability is read from
  `inventory_items`/`inventory_levels` when stock levels exist, with the legacy variant
  quantity used only as a compatibility fallback.
- Provide a module-owned Leptos admin UI package in `admin/` for inventory visibility,
  low-stock triage, stock-health inspection, targeted set-quantity corrections, +/-1 adjustments, reserve/release reservation transport coverage, and availability validation.
- Treat the backorder policy value `continue` case-insensitively across service write results,
  set/adjust/reserve/check-availability guardrails, admin read-side stock state, and
  commerce checkout/storefront compatibility paths through the exported policy helper.
- Own public-channel inventory visibility/projection helpers (`normalize_public_channel_slug`,
  channel-visibility metadata parsing, channel-visible available quantity loaders, and
  `PublicChannelInventoryProjection` / `PublicChannelInventoryVariantProjectionInput`) consumed
  by the umbrella commerce storefront/checkout compatibility layer so commerce adapters do not
  duplicate backorder policy branching.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs, entities, and errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- `apps/admin` consumes `rustok-inventory-admin` through manifest-driven composition;
  the admin package now uses native Leptos server functions backed by
  `AdminInventoryReadService` as the only read transport, with the previous transitional
  commerce GraphQL adapter removed, and uses native inventory-owned set/adjust/reserve/release
  quantity write endpoints plus check-availability validation for targeted stock corrections,
  +/-1 adjustments, reservation flows, and availability checks; set-quantity targets available
  quantity while preserving reserved units, while remaining non-admin write parity is split
  from the umbrella commerce surface.

## Entry points

- `InventoryModule`
- `InventoryService`
- `AdminInventoryReadService`
- public-channel inventory visibility/projection helpers exported from `services::public_channel`
- `rustok-inventory-admin`

See also `docs/README.md`.
