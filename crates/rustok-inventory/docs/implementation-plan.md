# Implementation plan for `rustok-inventory`

## Current state

`rustok-inventory` owns stock, reservations, availability, public-channel
inventory projections, and the inventory admin read/write surface. Commerce
uses owner public-channel availability for cart and checkout; it must not
recreate backorder, tenant/channel/locale, or stock lookup policy.

Inventory admin stock operations are owned by native/transport mutations:
set quantity, adjust quantity, reserve, release reservation, and check
availability. The admin package uses `HostRuntimeContext` and a typed
transactional event bus, is host-neutral, and intentionally has no GraphQL
fallback for this current operator surface.

`BootstrapService` owns default-location creation, initial item/level creation,
variant-record cleanup, and batched available-quantity reads when product
creates or deletes variants. This is a native transaction-sharing bootstrap
exception: no GraphQL/REST bootstrap contract exists, while public availability
and reservation contracts remain inventory-owned.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- FBA provider contract: `InventoryReservationPort` /
  `inventory.reservation.v1` in
  `crates/rustok-inventory/contracts/inventory-fba-registry.json`.
- Static and no-compile runtime evidence:
  `crates/rustok-inventory/contracts/evidence/inventory-contract-test-static-matrix.json`
  and `crates/rustok-inventory/contracts/evidence/inventory-runtime-contract-smoke.json`.
- `scripts/verify/verify-inventory-admin-boundary.mjs` locks the native
  core/transport/UI split and absence of pre-FFA/GraphQL admin paths.

## Open results

1. **Evolve locations, reservations, and availability as one owner contract.**
   Introduce any location or reservation semantics through inventory APIs and
   preserve explicit available/in-stock behavior rather than relying on legacy
   variant quantity.
   **Depends on:** the inventory persistence model and product variant contract.
   **Done when:** writes, reads, and public projections express the same
   location/reservation semantics with targeted integration coverage.

2. **Cover channel-aware availability edge cases.** Exercise backorder policy,
   missing/depleted levels, tenant/channel/locale context, and checkout/catalog
   visibility through the owner public-channel projection.
   **Depends on:** commerce checkout and storefront projection consumers.
   **Done when:** integration tests prove that cart, checkout, and storefront
   read models cannot diverge from `InventoryService` policy.

3. **Run the verification/CI evidence slice for `InventoryReservationPort`.**
   Execute the remote-adapter contract and fallback profiles before a
   `boundary_ready` promotion; retain native-only admin transport unless a
   public parity contract is introduced.
   **Depends on:** a runtime-composed commerce consumer and remote adapter
   environment.
   **Done when:** deadline, idempotency, typed-error, degraded-mode, and owner
   invocation evidence covers every published port operation.

## Verification

- `npm run verify:inventory:admin-boundary`
- `npm run verify:ecommerce:fba`
- `cargo xtask module validate inventory`
- `cargo xtask module test inventory`
- Targeted stock mutation, reservation, public-channel projection, and
  checkout-facing invariant tests.

## Change rules

1. Keep stock, reservation, and availability policy in this module.
2. Update local documentation, `rustok-module.toml`, and commerce documentation
   with any inventory/checkout/channel contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
