# Inventory durable reservation local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice extends the canonical `PersistentInventoryReservationIdentityPort` wrapper with
post-delegation local outcome diagnostics for:

- `InventoryReservationIdentityPort::reserve_inventory_by_identity`;
- `InventoryReservationIdentityPort::release_inventory_by_identity`.

Admission and tenant-validation diagnostics remain owned by the preceding durable reservation context
slice. This work covers stable errors returned after the unchanged persistent owner accepts the
original delegated context and request.

## Delegation order

Both operations now follow the same source order:

1. require write policy;
2. require write semantics;
3. parse the trimmed tenant UUID;
4. retain the accepted `PortContext` and safe request facts;
5. delegate the original context and request to the unchanged persistent owner;
6. inspect only a returned `PortError`;
7. emit a local diagnostic when its exact stable code and message identify a covered outcome;
8. return the same `PortError` unchanged.

Successful reserve, replay-adoption, already-released, and release snapshots do not emit a local
failure event.

## Retained request facts

Reserve diagnostics retain:

- reservation id;
- variant id;
- requested quantity;
- optional line-item id;
- external-id character length.

Release diagnostics retain:

- reservation id;
- external-id character length.

The caller-provided external-id text is deliberately not recorded. Validation can receive an empty or
oversized value before the persistent owner normalizes it, so logging only its character length retains
useful request evidence without publishing the raw identity string.

## Covered stable outcomes

The mapper requires exact `code + message` pairs. Code-only matching is intentionally forbidden.

| Public operation | Stable envelope | Local operation | Severity |
| --- | --- | --- | --- |
| both | `inventory.reservation_external_id_invalid` / `reservation external_id must contain 1 to 191 characters` | `normalize_external_id` | warning |
| reserve | `inventory.reservation_quantity_invalid` / `reservation quantity must be positive` | `validate_reservation_quantity` | warning |
| both | `inventory.variant_not_found` / `inventory variant was not found` | `load_variant` | warning |
| reserve | `inventory.state_not_found` / `variant has no configured inventory state` | `load_inventory_state` | warning |
| reserve | `inventory.reservation_identity_conflict` / `reservation identity is already bound to different reservation data` | `validate_existing_reservation_identity` | warning |
| reserve | `inventory.insufficient_inventory` / `insufficient inventory for reservation` | `reserve_available_stock` | warning |
| release | `inventory.reservation_not_found` / `inventory reservation was not found` | `load_reservation` | warning |
| release | `inventory.reservation_identity_conflict` / `reservation id is bound to another external identity` | `validate_release_external_identity` | warning |
| release | `inventory.reservation_item_missing` / `reservation inventory item is missing` | `load_reservation_inventory_item` | error |
| release | `inventory.reservation_identity_conflict` / `reservation identity changed while acquiring the owner lock` | `revalidate_release_identity` | warning |
| release | `inventory.reservation_ledger_inconsistent` / `inventory reservation ledger is inconsistent` | `release_reserved_quantity` | error |
| both | `inventory.available_quantity_overflow` / `inventory available quantity is outside the supported range` | `calculate_available_quantity` | error |
| both | `inventory.database_unavailable` / `inventory storage is temporarily unavailable` | `owner_storage` | error |

Unavailable, timeout, and invariant kinds use error severity. Validation, not-found, conflict, and other
ordinary caller or state rejection use warning severity.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_inventory`;
- exact public operation and local operation;
- boundary `inventory_reservation_identity_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- the safe request facts described above;
- stable code and message;
- typed error kind and retryability;
- the complete delegated `PortError` value.

## Pass-through behavior

The local mapper does not handle:

- write-policy rejection;
- missing write semantics;
- malformed tenant context;
- any unrecognized owner code or message;
- successful initial reserve;
- successful reserve replay adoption by reservation id or external id;
- successful insert-race adoption;
- successful first release;
- successful already-released replay.

Admission and tenant validation continue to be recorded before delegation by the existing wrapper
helpers. Unknown owner envelopes pass through without another event.

## Preserved owner behavior

This work does not change:

- request or response DTOs;
- public codes, messages, kinds, or retryability;
- external-id trimming and length limits;
- positive quantity validation;
- tenant-scoped variant loading;
- inventory-state and row-lock behavior;
- active-location selection and backorder policy;
- reservation id and external-id replay adoption;
- insert-race rollback and adoption;
- metadata construction;
- exact release identity checks;
- already-released idempotency;
- reservation ledger mutation and consistency checks;
- available-quantity calculation;
- database mapping;
- factory names or checkout composition.

The persistent owner implementation in `ports.rs` remains unchanged.

## Static evidence

`scripts/verify/verify-inventory-reservation-local-context.mjs` guards:

- admission → tenant validation → context retention → owner delegation → local mapping ordering;
- retained safe request facts for reserve and release;
- unchanged delegated method calls;
- exact stable code-and-message classification;
- operation-specific reserve and release local labels;
- technical versus ordinary severity;
- complete context and error fields;
- absence of raw external-id diagnostics;
- same delegated `PortError` return;
- pass-through of unknown, admission, and context errors;
- unchanged persistent owner validation, lookup, replay, stock, not-found, storage, and invariant branches.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- direct callers that bypass the canonical inventory factories where a bypass remains possible;
- remaining payment execution and compensation consumers;
- GraphQL customer reads and shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-inventory-reservation-local-context.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-inventory-availability-quantity-local-context.mjs
node scripts/verify/verify-commerce-checkout-inventory-boundary-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-inventory --lib
cargo check -p rustok-commerce --lib
```
