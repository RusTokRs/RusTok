# Inventory availability and quantity local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice extends the canonical `InProcessInventoryReservationPort` adapter with post-delegation
local outcome diagnostics for:

- `InventoryReservationPort::check_availability`;
- deprecated `InventoryReservationPort::reserve_inventory`;
- deprecated `InventoryReservationPort::release_inventory_reservation`.

Admission and tenant-validation diagnostics remain owned by the preceding adapter slice. This work
covers only stable errors returned after the unchanged `InventoryService` owner implementation has
accepted the delegated request.

## Delegation order

Each operation now follows the same source order:

1. require the existing read or write admission;
2. parse the trimmed tenant UUID;
3. retain a clone of the accepted `PortContext` plus the request variant and quantity;
4. delegate the original context and request to the unchanged owner implementation;
5. inspect only a returned `PortError`;
6. emit a local diagnostic when its exact stable code and message identify a covered owner outcome;
7. return the same `PortError` unchanged.

Successful snapshots are returned directly and do not emit a local failure event.

## Covered stable outcomes

The mapper requires exact `code + message` pairs. Code-only matching is intentionally forbidden.

| Public operation | Stable envelope | Local operation | Severity |
| --- | --- | --- | --- |
| `check_availability` | `inventory.validation` / `inventory request is invalid` | `validate_availability_request` | warning |
| `reserve_inventory` | `inventory.validation` / `inventory request is invalid` | `validate_reservation_request` | warning |
| `release_inventory_reservation` | `inventory.validation` / `inventory request is invalid` | `validate_reservation_release_request` | warning |
| all three | `inventory.variant_not_found` / `inventory variant was not found` | `load_variant` | warning |
| `reserve_inventory` only | `inventory.insufficient_inventory` / `inventory reservation conflicts with available stock` | `reserve_available_stock` | warning |
| all three | `inventory.database_unavailable` / `inventory storage is temporarily unavailable` | `owner_storage` | error |
| all three | `inventory.invariant_violation` / `inventory operation violated an owner invariant` | `owner_invariant` | error |

The release validation label is deliberately broad. The legacy owner maps negative release quantity,
insufficient reserved level quantity, and insufficient tracked reservation-item quantity to the same
stable public validation envelope, so the adapter does not invent a more specific reason after that
internal distinction has been sanitized.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_inventory`;
- exact public operation and local operation;
- boundary `inventory_reservation_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- variant id and requested quantity;
- stable code and message;
- typed error kind and retryability;
- the complete delegated `PortError` value.

Unavailable, timeout, and invariant kinds use error severity. Ordinary validation, not-found, and
stock-conflict outcomes use warning severity.

## Pass-through behavior

The local mapper does not handle:

- read/write policy rejection;
- missing write semantics;
- malformed tenant context;
- any unrecognized owner code or message;
- successful availability results, including `available = false`;
- successful zero-quantity reserve or release snapshots.

Those results preserve the preceding behavior without another local event. Admission and tenant
validation continue to be recorded before delegation by the existing adapter helpers.

## Preserved owner behavior

This work does not change:

- request or response DTOs;
- public codes, messages, kinds, or retryability;
- quantity validation rules;
- variant lookup or tenant isolation;
- channel-aware availability policy;
- backorder behavior;
- legacy reservation persistence;
- legacy release ordering across reservation items and inventory levels;
- database or invariant mapping;
- checkout composition or factory exports;
- durable identity reservation behavior.

The original owner implementation in `ports.rs` and `services/inventory.rs` remains unchanged.

## Static evidence

`scripts/verify/verify-inventory-availability-quantity-local-context.mjs` guards:

- admission → tenant validation → context retention → owner delegation → local mapping ordering;
- retained context, variant id, and quantity for every operation;
- unchanged delegated method calls;
- exact stable code-and-message classification;
- operation-specific validation labels;
- reserve-only insufficient-stock classification;
- technical versus ordinary severity;
- complete context and error fields;
- same delegated `PortError` return;
- pass-through of unknown and context/admission errors;
- unchanged legacy stable envelope construction;
- unchanged owner validation, variant lookup, stock-conflict, and release-validation source branches.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- direct external callers that intentionally construct `InventoryService` as
  `InventoryReservationPort` instead of using the canonical factory;
- durable reservation local request, identity, not-found, stock, storage, and ledger outcomes;
- remaining payment execution and compensation consumers;
- GraphQL customer reads and shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-inventory-availability-quantity-local-context.mjs
node scripts/verify/verify-inventory-availability-quantity-context.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-inventory --lib
cargo check -p rustok-commerce --lib
```
