# Documentation `rustok-inventory`

`rustok-inventory` is the default inventory submodule of the `ecommerce` family.

## Purpose

- inventory service logic;
- stock-related migrations;
- `InventoryModule`, `InventoryService`, backend `AdminInventoryReadService` and native admin stock write endpoints;
- module-owned admin UI package `rustok-inventory/admin` for inventory visibility,
  low-stock triage and variant-level stock inspection.

## Scope

- runtime dependency: `product`;
- the module owns the inventory/stock boundary and the operator read-side UI surface
  for stock levels;
- the backend read-side for the admin now has an inventory-owned service/DTO in
  `src/services/admin_read.rs`, which returns a tenant-scoped product/variant/price/translations
  model for native server-function read transport;
- admin UI read-side now goes only through inventory-owned `admin/src/core.rs`, `admin/src/transport/mod.rs`, explicit native `#[server]` functions in `admin/src/transport/native_server_adapter.rs`, and explicit Leptos adapter `admin/src/ui/leptos.rs`; commerce GraphQL fallback, `admin/src/transport.rs`, pre-FFA `admin/src/api.rs`, `rustok-graphql`, and token/tenant-slug fallback parameters are absent;
- dedicated native inventory write/validation endpoints `inventory/variant/set-quantity`,
  `inventory/variant/adjust-quantity`, `inventory/variant/reserve-quantity`,
  `inventory/variant/release-reservation` and `inventory/variant/check-availability` have already been extracted
  to a module-owned surface without GraphQL selected path and return typed write/validation results;
  set-quantity treats the requested quantity as the target available quantity and preserves
  the existing reserved stock, while backorder policy `continue` is normalized case-insensitively
  in the service/read-side and commerce checkout/storefront compatibility semantics through an exported
  inventory-owned policy helper; further non-admin/channel-aware parity is handled separately from the admin UI scope;
- public-channel inventory visibility/projection helpers (`normalize_public_channel_slug`, metadata allowlist parsing, channel-visible available quantity loaders, `PublicChannelInventoryProjection` / `PublicChannelInventoryVariantProjectionInput` and `load_inventory_projection_by_variant_for_public_channel`) belong to the inventory crate and are reused by the umbrella `rustok-commerce` for storefront/checkout compatibility without duplicating backorder policy branching in the commerce DTO adapter;
- `BootstrapService` is the inventory-owned native transaction-sharing contract for
  product variant bootstrap; it creates default locations and initial inventory rows and
  loads available quantities without transferring inventory persistence ownership to product;
- common DTOs, entities and error surface come from `rustok-commerce-foundation`.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary
  without returning responsibility to the umbrella `rustok-commerce`;
- the inventory-owned backend admin read service is exported by the root crate and is the source
  for native server-function read transport;
- inventory-owned admin UX and read facade are published through `rustok-inventory/admin`;
  read-side and targeted set/adjust/reserve/release quantity plus check-availability flows go through native inventory-owned server-function surface without commerce GraphQL selected path;
- cross-module contract changes must be synchronized with `rustok-commerce`
  and neighboring split modules.

## Verification

- `cargo xtask module validate inventory`
- `cargo xtask module test inventory`
- targeted commerce tests for the inventory domain when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)
