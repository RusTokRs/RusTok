# Documentation `rustok-fulfillment`

`rustok-fulfillment` is the default fulfillment submodule of the `ecommerce` family.

## Purpose

- schema `shipping_options`;
- schema `fulfillments`;
- schema `fulfillment_items`;
- `FulfillmentModule` and `FulfillmentService`;
- shipping boundary for the checkout chain `cart -> payment -> order -> fulfillment`;
- first-class `allowed_shipping_profile_slugs` in the shipping-option contract, which for now normalizes into metadata-backed `shipping_profiles.allowed_slugs`;
- transport-level validation for `allowed_shipping_profile_slugs` now lives in the `rustok-commerce` facade and checks references against active shipping profiles from the typed registry `shipping_profiles`;
- storefront cart/checkout no longer relies on a single global shipping option: `rustok-commerce` over this boundary already builds `delivery_groups[]`, typed `shipping_selections[]` and multi-fulfillment checkout, while singular shipping fields remain only as a compatibility shim for single-group carts;
- typed `fulfillment_items[]` now capture the composition of each fulfillment over `order_line_item_id + quantity`, so the post-order delivery path no longer has to restore item scope only from metadata delivery group;
- typed `fulfillment_items[]` now also hold `shipped_quantity` and `delivered_quantity`, so partial ship/deliver progress lives within the fulfillment boundary itself, not in ad-hoc metadata outside the model;
- admin/manual post-order create path in the `rustok-commerce` facade is now built over the same typed `fulfillment_items[]` and validates order-line ownership + remaining quantity before calling `FulfillmentService`;
- `ship_fulfillment` and `deliver_fulfillment` now accept item-level quantity adjustments, persist only language-agnostic audit events in fulfillment/item metadata and support partial post-order delivery progress without a separate OMS layer; `delivered_note` remains a typed fulfillment field;
- explicit `reopen_fulfillment` and `reship_fulfillment` now also live in this boundary, so post-order delivery recovery no longer requires implicit status hacks and does not put language-dependent business text into metadata;
- admin REST/admin GraphQL and the module-owned `rustok-fulfillment/admin` UI already consume this shipping-option contract as a typed operator surface over `FulfillmentService`, including deactivate/reactivate lifecycle over the `active` flag;
- built-in manual/default fulfillment flow at the current stage;
- fulfillment-owned provider SPI registry for external carrier composition: descriptor/adapter id validation, health/degraded-mode registration guards and side-effect-free runtime-mode checks before adapter invocation.

## Scope

- the module does not depend on the `rustok-commerce` umbrella to avoid creating a cycle;
- the module does not own the order or customer profile, only references them by identifiers;
- provider-specific delivery is deferred to the backlog and should live as the next nested submodule over the fulfillment boundary, not mixed with the base shipping model;
- GraphQL and REST transport remain in the `rustok-commerce` facade for now.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- transport endpoint shell and GraphQL surface are still published through `rustok-commerce`, admin UI ownership has already been extracted to `rustok-fulfillment/admin`, and storefront presentation/selection DTO/core/selection-update materialization/transport-fallback ownership — to `rustok-fulfillment/storefront`;
- cross-module contract changes must be synchronized with `rustok-commerce` and neighboring split modules.

## FFA split for admin

The admin package now uses framework-agnostic defaults `admin/src/core.rs`, a facade `admin/src/transport.rs` over GraphQL shipping-option transport and an explicit Leptos render adapter `admin/src/ui/leptos.rs`; the crate root only connects the module layers and re-exports `FulfillmentAdmin`.

## Conditional capability boundary

Verification must include rejection of digital lines in fulfillment creation, exact
coverage of physical lines, mixed-order subset handling, and provider absence that
does not break digital-only Commerce flows. The governing capability classification is
in [`docs/architecture/settings.md`](../../../../docs/architecture/settings.md).

## Verification

- cargo xtask module validate fulfillment
- cargo xtask module test fulfillment
- targeted commerce tests for the fulfillment domain when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [README admin UI](../admin/README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)

## Shipment source continuation

The [prepared stock API](../CRATE_API.md#prepared-shipment-stock-and-recovery-2026-11-01-source-only)
and [Commerce source handoff](../../rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence)
record F-owned lifecycle/counters, prepared consumer protocol, pending-only cancel and literal-key
local recovery. Source implementation is uncompiled/unexecuted, with no new peer dependency or
readiness promotion. The targeted recheck adds provider/key pinning, monotonic retained outcomes,
actual nullable reopening status and final remaining-budget gates; see the same handoff.
Native and transport/runtime regression execution stays with the maintainer.

## Native stock invalidations

I's native implementation now publishes the existing stock invalidation on F's supplied
write transaction, without moving lifecycle or publisher ownership into the host/C. See the
[Inventory API](../../rustok-inventory/CRATE_API.md#native-stock-outbox-2026-11-01-source-only)
and [source evidence](../../rustok-inventory/contracts/evidence/native-stock-outbox-source.json).
Preparation/no movement publish none; all added regression definitions remain unrun and no
relay, Index or step-8 readiness is asserted.
