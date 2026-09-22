# Checkout inventory boundary context

Status: **source-ready / unvalidated**

## Scope

This slice retains the complete inventory `PortContext` when the durable checkout inventory
reservation executor receives an owner error or detects an invalid owner response.

The covered owner operations are:

- `reserve_inventory_by_identity`;
- `release_inventory_by_identity`.

Both the direct `PortError` paths and the synthetic response-mismatch paths now pass through the
same structured diagnostic boundary before the existing reservation journal write.

## Retained context

The diagnostic event records:

- truthful owner: `rustok_inventory`;
- correlation and tenant identity;
- actor, channel and locale;
- causation and traceparent;
- idempotency key and deadline;
- exact owner operation;
- cart line and reservation identity;
- original port code, typed kind and retryability;
- explicit boundary: `commerce_checkout_inventory_reservation`.

Unavailable, timeout and invariant failures use error severity. Validation, not-found, conflict and
forbidden failures use warning severity.

## Compatibility

The following behavior is intentionally unchanged:

- reservation planning and replay adoption;
- immutable cart snapshot validation;
- reserve and release request DTOs;
- reservation identity and external-id semantics;
- provider response validation;
- journal `record_error` code and message payloads;
- `Boundary` and `BoundaryAndJournal` public error variants;
- retryability propagation;
- checkpoint and release state transitions;
- operation, reservation and inventory owner ports.

The same cloned `PortContext` is used for the owner call and the subsequent diagnostic event. No
technical cause is added to the public error or journal payload.

## Plan alignment

This advances the open ecommerce P0 correlation-safe inventory mapper/adapter cleanup. It closes
consumer-side context loss at the durable checkout inventory reserve/release boundary.

Still open:

- inventory owner read/write admission diagnostics outside this executor;
- checkout inventory availability consumer context;
- remaining inventory HTTP/GraphQL/admin adapters;
- request-context propagation gaps in other ecommerce owners;
- runtime and cross-backend evidence.

No FBA or FFA status is promoted by this source-only slice.

## Intended validation

```bash
node scripts/verify/verify-commerce-checkout-inventory-boundary-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```

Tests, Cargo commands, formatting commands, verifier execution, workflow checks and CI were not run
for this slice, per maintainer instruction.
