# Inventory reservation owner context

Status: **source-ready / unvalidated**

## Scope

This slice closes the write-admission and tenant-context diagnostic gap at the durable inventory
reservation owner boundary.

The covered public operations are:

- `InventoryReservationIdentityPort::reserve_inventory_by_identity`;
- `InventoryReservationIdentityPort::release_inventory_by_identity`.

These operations are used by the durable checkout inventory reservation and compensation paths. The
deprecated quantity-only reserve/release methods and the availability read operation remain outside
this narrow slice.

## Public API compatibility

The crate-root API retains the existing names:

- `InventoryReservationIdentityPort`;
- `InventoryIdentityReservationRequest`;
- `InventoryIdentityReservationReleaseRequest`;
- `InventoryIdentityReservationSnapshot`;
- `InventoryIdentityReservationReleaseSnapshot`;
- `PersistentInventoryReservationIdentityPort`;
- `PersistentInventoryReservationIdentityPort::new`;
- `in_process_inventory_reservation_identity_port`.

The public `rustok_inventory::ports` module path also retains the same contracts, struct, and factory.

The original `ports.rs` source is compiled as the private `ports_impl` module. A public compatibility
facade selectively re-exports all existing inventory port contracts while exporting the new durable
reservation wrapper under the original struct and factory names. External callers therefore cannot
construct or select the direct durable reservation implementation through either the crate root or
the public `ports` path.

## Ordering

Each durable reservation write now flows through these layers:

1. require write policy;
2. require write semantics;
3. parse the trimmed tenant UUID;
4. delegate the original `PortContext` and request to the unchanged persistent owner;
5. allow the persistent owner to repeat its existing admission and tenant checks before storage work.

The repeated checks are deterministic. The wrapper uses the same `PortCallPolicy::write()` contract,
the same write-semantics requirements, and the same trimmed tenant UUID parsing rule as the existing
owner implementation.

No actor validation was added because the legacy durable reservation owner did not reject malformed
actor identity. Adding that check would change request acceptance rather than retain diagnostics.

## Admission diagnostics

Write-policy and write-semantics failures now record:

- truthful owner `rustok_inventory`;
- exact operation;
- admission phase `policy` or `write_semantics`;
- boundary `inventory_reservation_identity_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable code and message;
- typed error kind and retryability;
- the original `PortError`.

Unavailable, timeout, and invariant admission failures use error severity. Ordinary validation,
forbidden, conflict, and other caller rejection use warning severity.

The wrapper returns the exact admission `PortError` unchanged.

## Tenant-context validation

The wrapper preserves the existing validation contract:

- trim the delegated tenant string;
- parse it as a UUID;
- return code `inventory.context_invalid`;
- return message `inventory request context is invalid`.

For a parse failure, the structured warning retains the UUID parse cause together with the complete
delegated `PortContext`, truthful owner, exact operation, validation phase `tenant_id`, stable public
envelope, and boundary.

The same constructed validation `PortError` is returned unchanged.

## Preserved durable reservation behavior

This slice does not change:

- reservation request or snapshot DTOs;
- reservation and external identity semantics;
- external-id trimming or length bounds;
- positive quantity validation;
- tenant-scoped variant loading;
- inventory-item locking;
- active-location selection;
- backorder policy;
- atomic reserved-quantity updates;
- replay adoption by reservation id or external id;
- conflicting replay classification;
- reservation metadata;
- insert-race adoption;
- exact release identity checks;
- already-released idempotency;
- reservation ledger consistency checks;
- storage mappings;
- public codes, messages, kinds, or retryability.

The original owner receives the original `PortContext` and request after wrapper acceptance.

## Static evidence

`scripts/verify/verify-inventory-reservation-owner-context.mjs` guards:

- private legacy implementation and public compatibility facade;
- preserved crate-root and module-path contracts;
- wrapper constructor and factory cutover;
- exact reserve and release operations;
- admission → tenant validation → owner delegation ordering;
- write-policy and write-semantics interception;
- full admission context and severity policy;
- same admission error return;
- trimmed tenant parsing and retained parse cause;
- stable tenant validation envelope;
- full tenant-validation context and same error return;
- preserved legacy external-id, quantity, identity-conflict, and ledger-invariant behavior.

The existing checkout inventory boundary verifier remains applicable because commerce continues to
call the same root factory and owner trait. The wrapper only adds owner-side diagnostics before the
unchanged persistent owner call.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- deprecated quantity reservation write admission and context diagnostics;
- inventory availability read admission and context diagnostics;
- durable reservation local request, identity, not-found, stock, and ledger outcomes beyond the
  admission/context scope of this slice;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and the shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-commerce-checkout-inventory-boundary-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-inventory --lib
cargo check -p rustok-commerce --lib
```
