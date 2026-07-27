# Inventory availability and quantity owner context

Status: **source-ready / unvalidated**

## Scope

This work publishes a context-preserving owner adapter for the non-durable inventory reservation
contract:

- `InventoryReservationPort::check_availability`;
- deprecated `InventoryReservationPort::reserve_inventory`;
- deprecated `InventoryReservationPort::release_inventory_reservation`.

The adapter closes source-level admission and tenant-validation context loss for callers that use the
canonical factory. Every repository checkout composition now uses that factory:

- the compact `JournaledCheckoutService` compatibility path;
- the mounted staged storefront runtime used by storefront transports;
- the public legacy storefront checkout compatibility wrapper.

No repository checkout composition constructs `InventoryService` directly as an
`InventoryReservationPort` dependency after this cutover.

## Canonical API

The crate root and `rustok_inventory::ports` facade export:

- `InProcessInventoryReservationPort`;
- `in_process_inventory_reservation_port`.

The factory accepts the same database connection and transactional event bus required by
`InventoryService`, constructs the existing owner service internally, and returns
`Arc<dyn InventoryReservationPort>`.

Existing contracts remain unchanged:

- `InventoryReservationPort`;
- availability, reservation, and release request DTOs;
- availability, reservation, and release snapshots;
- the root `InventoryService` concrete service;
- the durable identity reservation wrapper and factory added by the preceding inventory slice.

## Ordering

### Availability read

`check_availability` flows through the canonical adapter as:

1. require read policy;
2. parse the trimmed tenant UUID;
3. delegate the original `PortContext` and request to the existing `InventoryService` trait
   implementation;
4. allow the existing implementation to repeat its deterministic policy and tenant checks before
   invoking availability policy.

The adapter does not require write semantics for the read operation.

### Deprecated quantity mutations

`reserve_inventory` and `release_inventory_reservation` flow as:

1. require write policy;
2. require write semantics;
3. parse the trimmed tenant UUID;
4. delegate the original `PortContext` and request to the existing implementation.

The repeated inner checks use the same policy, write-semantics, and tenant parsing rules, so accepted
behavior is unchanged.

## Admission diagnostics

Read-policy, write-policy, and write-semantics failures record:

- truthful owner `rustok_inventory`;
- exact owner operation;
- admission phase `policy` or `write_semantics`;
- boundary `inventory_reservation_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable error code and message;
- typed error kind and retryability;
- the original `PortError`.

Unavailable, timeout, and invariant failures use error severity. Ordinary caller rejection uses
warning severity.

The exact admission `PortError` is returned unchanged.

## Tenant validation

The adapter preserves the existing tenant validation contract:

- trim `PortContext.tenant_id`;
- parse it as a UUID;
- return code `inventory.context_invalid`;
- return message `inventory request context is invalid`.

A parse failure retains the UUID parse cause together with the complete delegated context, truthful
owner, exact operation, validation phase `tenant_id`, stable public envelope, and boundary. The
constructed validation `PortError` is returned unchanged.

No actor validation was added. The existing owner does not reject malformed actor identity, so adding
that rule would alter request acceptance rather than retain diagnostics.

## Checkout composition cutover

### Journaled compatibility

`JournaledCheckoutService` previously constructed `InventoryService` directly for checkout-plan
availability reads. It now calls `in_process_inventory_reservation_port` and passes the returned trait
object to `CheckoutPlanBuilder`.

### Mounted staged storefront

`storefront_staged_checkout_runtime.rs` previously constructed `InventoryService` directly for the
mounted checkout-plan availability owner. It now calls the same canonical factory with the existing
runtime database connection and cloned transactional event bus.

The staged runtime continues to use `in_process_inventory_reservation_identity_port` for durable
reserve/release. Atomic cart, product, marketplace allocation, marketplace commission, marketplace
ledger, payment, compensation, and recovery composition remain unchanged.

### Legacy storefront compatibility

The public `storefront_checkout_runtime::complete_storefront_checkout` wrapper remains available for
compatibility and keeps its existing cart access check, storefront repricing, actor resolution, and
`CheckoutService` delegation. Only the inventory dependency passed to `CheckoutService::new` changed:
it now comes from `in_process_inventory_reservation_port` using the same runtime database connection
and transactional event bus.

The function remains marked `#[allow(dead_code)]`; this cutover does not remove or rename the public
compatibility API.

## Preserved owner behavior

This work does not change:

- channel-aware availability policy;
- requested quantity handling;
- deprecated quantity reserve semantics;
- deprecated quantity release semantics;
- backorder policy;
- variant-not-found behavior;
- insufficient inventory classification;
- validation mapping;
- database and invariant mapping;
- request or response DTOs;
- public codes, messages, kinds, or retryability;
- durable identity reservation behavior from the preceding inventory slice;
- staged checkout constructor contracts or public transport convergence;
- legacy storefront cart access, repricing, actor resolution, checkout input, or public function
  signature.

The original context and request are delegated after adapter acceptance.

## Static evidence

`scripts/verify/verify-inventory-availability-quantity-context.mjs` guards:

- crate-root and `ports` facade exports;
- preserved durable identity exports;
- canonical adapter constructor and factory;
- exact availability, reserve, and release operations;
- read admission without write semantics;
- write-policy and write-semantics interception;
- admission → tenant validation → owner delegation ordering;
- full admission context and severity classification;
- same admission error return;
- trimmed tenant parsing, retained parse cause, stable envelope, and same error return;
- unchanged legacy availability and quantity service calls and owner error mapping;
- journaled compatibility cutover;
- mounted staged storefront cutover with database/event-bus delegation;
- preserved durable reservation factory and staged plan-builder composition;
- legacy storefront public compatibility, cart access, repricing, actor resolution, region/cart/product
  composition, and canonical inventory factory;
- absence of direct `InventoryService` construction in every repository checkout composition.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- direct external callers that intentionally construct `InventoryService` as
  `InventoryReservationPort` rather than using the canonical factory;
- local availability/quantity request and owner outcomes beyond admission and tenant validation;
- durable reservation local request, identity, not-found, stock, and ledger outcomes;
- remaining payment execution and compensation consumers;
- GraphQL customer reads and shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-inventory-availability-quantity-context.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-inventory --lib
cargo check -p rustok-commerce --lib
```
