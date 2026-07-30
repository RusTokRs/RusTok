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

Mounted Inventory admin native endpoints use static public error envelopes for
framework extraction, runtime dependency, read, and write failures. Original
owner causes remain in structured diagnostics with per-call correlation,
tenant, actor, subject, and request channel/locale context when available. The
source contract is guarded by
`scripts/verify/verify-inventory-admin-native-error-safety.mjs`; runtime evidence
has not been executed or promoted.

The Inventory admin client transport facade independently fails closed after the
native call. All eight read/write operations create a per-call correlation
context, retain the original `ServerFnError` only in structured diagnostics, and
return one static `InventoryTransportError` message. Tenant, product, variant,
locale, filter, and numeric request values are represented only by safe
presence/length shape facts. This source contract is guarded by
`scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs` and
remains unvalidated.

`BootstrapService` owns default-location creation, initial item/level creation,
variant-record cleanup, and batched available-quantity reads when product
creates or deletes variants. This is a native transaction-sharing bootstrap
exception: no GraphQL/REST bootstrap contract exists, while public availability
and reservation contracts remain inventory-owned.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Admin native error safety: `source_ready_unvalidated`
- Admin client transport error safety: `source_ready_unvalidated`
- FBA provider contract: `InventoryReservationPort` /
  `inventory.reservation.v1` in
  `crates/rustok-inventory/contracts/inventory-fba-registry.json`.
- Static and no-compile runtime evidence:
  `crates/rustok-inventory/contracts/evidence/inventory-contract-test-static-matrix.json`
  and `crates/rustok-inventory/contracts/evidence/inventory-runtime-contract-smoke.json`.
- `scripts/verify/verify-inventory-admin-boundary.mjs` locks the native
  core/transport/UI split and absence of pre-FFA/GraphQL admin paths.
- `scripts/verify/verify-inventory-admin-native-error-safety.mjs` locks mounted
  static public envelopes and private owner-cause diagnostics without claiming a
  runtime pass.
- `scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs` locks
  the final facade mapping, per-operation correlation context, safe request-shape
  diagnostics, and static `InventoryTransportError` text without claiming a
  runtime pass.

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

3. **Run the verification/CI evidence slice for `InventoryReservationPort` and
   admin native/client error safety.** Execute the remote-adapter contract and
   fallback profiles before a `boundary_ready` promotion; retain native-only
   admin transport unless a public parity contract is introduced. Execute both
   focused admin error-safety guards, compile the default/hydrate/SSR package,
   and retain mounted browser plus server-function failure evidence before
   promoting either source-only status.
   **Depends on:** a runtime-composed commerce consumer and remote adapter
   environment.
   **Done when:** deadline, idempotency, typed-error, degraded-mode, owner
   invocation, client-facade sanitization, and mounted public-envelope evidence
   covers every published port operation and native admin endpoint.

## Verification

- `npm run verify:inventory:admin-boundary`
- `node scripts/verify/verify-inventory-admin-native-error-safety.mjs`
- `node scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs`
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
