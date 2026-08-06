# Checkout payment compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted Commerce checkout compensation facade is
`checkout_compensation_payment_safe.rs`. It retains
`checkout_compensation_owner_ports.rs` privately and now adapts payment, order,
and inventory owner errors before they reach the retained Commerce mapper.

This document records the payment side of the combined
payment/order/inventory facade. Cart compensation remains open.

## Payment boundary policy

The default payment factory, provider-registry constructor, and custom payment
port injection are all wrapped. The wrapper preserves the canonical request and
response, delegated `PortContext`, owner `PortError.kind`, exact `code`,
`retryable`, successful snapshot, and compensation ordering.

Owner message text is replaced with a Commerce-owned static message before
`CheckoutCompensationError` construction or
`mark_compensation_retryable` persistence. The manual-reconciliation owner code
receives `Checkout payment compensation requires manual reconciliation`.

## Diagnostics

The facade emits a redacted token plus bounded context shapes, identifier
shapes, opaque-text presence/length, metadata kind/count, typed classification,
and owner-message presence/length. It does not emit the complete `PortError`,
owner message text, raw context values, reason text, or metadata values.

Retained compatibility diagnostics are suppressed for truthful
`rustok_payment`, `rustok_order`, and `rustok_inventory` labels. The retained
cart diagnostic remains active.

## Preserved behavior

The retained compensation source, public service signatures, owner calls,
operation-journal transitions, and payment-before-order-before-inventory-before-cart
ordering remain unchanged.

Order and inventory compensation are now also adapted. Cart compensation mapper
cleanup remains open, so broad non-payment compensation cleanup is not closed.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, payment-provider calls,
database scenarios, restart scenarios, remote-port scenarios, workflows, or CI
were executed.

Suggested maintainer checks:

```bash
node scripts/verify/verify-commerce-checkout-payment-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-order-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-compensation-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
cargo check -p rustok-commerce --lib
```
