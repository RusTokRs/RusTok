# Inventory availability and quantity local diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens post-delegation diagnostics in the canonical
`InProcessInventoryReservationPort` adapter for:

- `InventoryReservationPort::check_availability`;
- deprecated `InventoryReservationPort::reserve_inventory`;
- deprecated `InventoryReservationPort::release_inventory_reservation`.

The existing exact `code + message` classification remains unchanged. Unknown owner envelopes still
pass through without an additional local event, and the same delegated `PortError` is returned.

## Preserved delegation order

Each operation still performs admission, trimmed tenant validation, accepted-context/request-shape
retention, unchanged owner delegation, covered-error classification, and same-error return in that
order. Successful snapshots do not emit a local failure event.

## Covered stable outcomes

The preserved local operations are:

- `validate_availability_request`;
- `validate_reservation_request`;
- `validate_reservation_release_request`;
- `load_variant`;
- reserve-only `reserve_available_stock`;
- `owner_storage`;
- `owner_invariant`.

Unavailable, timeout, and invariant outcomes retain error severity. Validation, not-found, and
stock-conflict outcomes retain warning severity.

## Bounded diagnostic shape

Covered events retain:

- truthful owner, exact public operation, local operation, correlation ID, and boundary;
- tenant/actor/channel/locale/causation/trace/idempotency/deadline shape facts;
- variant UUID non-nil shape;
- quantity zero and negative shape;
- stable covered code, error-message length, retryability, and technical-failure classification.

They no longer record the complete delegated `PortError`, raw tenant/actor/channel/locale values,
causation or tracing tokens, idempotency keys, raw variant UUIDs, exact quantities, public error
message text, or debug-formatted error kinds.

## Preserved owner behavior

Request and response DTOs, quantity validation, variant lookup, tenant isolation, channel-aware
availability, backorder policy, legacy quantity persistence/release ordering, public error envelopes,
and canonical checkout composition are unchanged. The owner implementation in `ports.rs` and
`services/inventory.rs` is not modified.

## Evidence

- `crates/rustok-inventory/contracts/evidence/availability-quantity-diagnostic-safety-source-review.json`
- `scripts/verify/verify-inventory-availability-quantity-local-context.mjs`
- `scripts/verify/verify-inventory-availability-quantity-context.mjs`

## Remaining boundary

This closes the identified raw diagnostic payloads in
`reservation_port_context.rs`. The broader ecommerce correlation-safe mapper task remains open for
payment compensation, fulfillment, customer, tax, promotion, ecommerce adapters, and remaining
non-`PortError` public envelopes.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed. No compile or
runtime status is promoted.
