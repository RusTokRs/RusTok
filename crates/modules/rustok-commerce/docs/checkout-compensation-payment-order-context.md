# Checkout compensation owner context

Status: **source-reviewed / unvalidated**

## Scope

The mounted `checkout_compensation_error_safe.rs` facade retains bounded consumer
context for payment, order, inventory, cart snapshot, and cart release owner calls.
The private `checkout_compensation_owner_ports.rs` implementation and its
compensation ordering remain unchanged.

## Delivered context contract

Each owner adapter receives the complete original `PortContext` and request, then
delegates them unchanged. On failure it records only:

- tenant and actor identity shapes;
- actor kind, claim count, and role count;
- channel, locale, correlation, causation, traceparent, and idempotency shapes;
- deadline;
- operation-specific UUID shapes;
- optional text presence/length and metadata kind/count where applicable;
- exact owner code, typed kind, retryability, and message presence/length;
- a redacted diagnostic token.

The adapters do not record the complete `PortError`, owner message text, raw
context values, request identifiers, reasons, metadata, or locale text.

## Owner operations

- payment: `compensate_checkout_payment`;
- order: `compensate_checkout_order`;
- inventory: `release_inventory_by_identity`;
- cart read: `read_cart_checkout_snapshot`;
- cart release: `release_cart_checkout`.

Retained compatibility events are suppressed only for truthful labels
`rustok_payment`, `rustok_order`, `rustok_inventory`, and `rustok_cart` because
those calls have already emitted bounded adapter diagnostics.

## Preserved behavior

This work does not change request DTOs, delegated contexts, owner responses,
manual-reconciliation code recognition, compensation ordering, lifecycle checks,
claim/lease/retry control flow, or journal mutation ordering.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, owner scenarios, database
scenarios, restart scenarios, remote-port scenarios, workflows, or CI were run.
