# Documentation `rustok-inventory`

`rustok-inventory` is the default inventory submodule of the `ecommerce` family.

Current prerequisite: [history-preserving Product/Variant deletion](./implementation-plan.md#catalog-deletion-safety-2026-11-01-source-only)
with 31 Rust/10 Node definitions uncompiled/unrun, not SKU retirement or an Inventory Index
source/tombstone. The [source packet](../contracts/evidence/catalog-deletion-source.json) records
read-only evidence separately from compiler, SQL, migration and runtime gates.

Previous source continuation: [reservation facts and acknowledged typed delivery](./implementation-plan.md#reservation-fact-delivery-continuation-2026-11-01-source-only),
following [Inventory-owned adoption](./implementation-plan.md#reservation-adoption-into-order-2026-10-31-source-work)
and the anonymous release prerequisite. Current context, canonical transactional facts and explicit
Search refetch are wired in source; regression sources remain uncompiled/unrun. Rust digests,
actual SQL/relay/Iggy/restart/Search, generic Index and Product step 8 are not certified.

## Purpose

- inventory service logic;
- stock-related migrations;
- `InventoryModule`, `InventoryService`, and native admin stock write endpoints;
- module-owned admin UI package `rustok-inventory/admin` for inventory visibility,
  low-stock triage and variant-level stock inspection.

## Scope

- runtime dependency: `product`;
- the module owns the inventory/stock boundary and the operator read-side UI surface
  for stock levels;
- the backend read-side for the admin routes through canonical Product catalog service DTOs
  for product/variant/price/translations and inventory-owned stock queries for native server-function read transport;
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
- the admin read transport delegates product catalog queries to the Product owner service and
  resolves stock levels from inventory;
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

## Shipment source continuation

The [native shipment handoff](../../rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence)
and [owner API](../CRATE_API.md#native-shipment-provenance-2026-11-01-source-only) record the
new bounded provenance/return-overlap implementation and additive migration. Source guards
are not SQL/locking, compiled regression or deployment evidence; Product step 8 stays open.
Native elapsed-budget gates now include awaited locks/reads, batch writes and return; failed
preparation still rolls back rather than retaining probes or refreshing the admitted budget.

## Native stock outbox continuation

The [native API](../CRATE_API.md#native-stock-outbox-2026-11-01-source-only) and
[plan/evidence](implementation-plan.md#native-stock-outbox-continuation-2026-11-01-source-only)
now include I-owned canonical invalidations in the physical write transaction. No new event
schema/public capability exists; preparation/exact replay/no movement do not publish. Global
availability snapshots are descriptive, never movement deltas. All 16 additional Rust/10 Node
cases remain unrun; Search source wiring is not delivery, generic Index or step-8 certification.

## Anonymous quantity prerequisite

The [current owner contract](../CRATE_API.md#anonymous-quantity-reservations-2026-11-01-source-only)
and [plan/evidence](./implementation-plan.md#anonymous-quantity-reservation-release-2026-11-01-source-only)
replace the independent quantity FIFO reducers with exact anonymous allocation/location effects.
Scoped selectors refuse instead of being ignored; keyed claims remain protected. The 27 Rust/10
Node regression definitions are unrun. Quantity operations remain non-idempotent; ordinary
reservation typed invalidations, delivery/consumers and Product step 8 remain unfinished.

## Canonical source delivery continuation (2026-11-02; uncompiled)

[Owner API](../CRATE_API.md#inventory-source-canonical-delivery-2026-11-02-source-continuation)
separates Changed from final BaselineActivated: retained UUID/version/current actor share the
actual caller transaction; staged pages do not announce ready SKUs. Product actors are forwarded
explicitly. Required module handling cannot silently succeed as broker-only delivery.
[Evidence](../contracts/evidence/source-delivery-source.json) lists 26 new Rust/10 Node definitions,
all unrun. Selected authoritative source/initial replay/Index and full operator-client recovery
remain pending. Reads never fabricate missing historical facts; no live baseline before real
handlers and operator recovery.
