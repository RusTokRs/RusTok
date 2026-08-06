# Checkout order compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted Commerce checkout compensation facade now adapts payment, order,
and inventory owner ports while retaining
`checkout_compensation_owner_ports.rs` unchanged as private business logic.

This document records the order side of the combined
payment/order/inventory facade. Cart compensation remains separate work.

## Order boundary policy

The facade wraps the default in-process order factory, the identity-aware
constructor, and custom `CheckoutOrderCompensationPort` injection. It delegates
the original context and request and preserves the successful snapshot,
`PortError.kind`, exact owner `code`, and `retryable`.

Before the retained mapper sees a failure, owner text is replaced with a static
message selected by `PortErrorKind`. The manual-reconciliation code receives
`Checkout order compensation requires manual reconciliation`. Owner text cannot
enter the public compensation error or retryable operation-journal message.

## Diagnostics

The adapter records only bounded context shapes, operation/subject/expected-id
shapes, opaque-text presence/length, owner classification, message
presence/length, and a redacted diagnostic token. It does not record complete
errors, owner text, raw context values, identifiers, or reason text.

Retained compatibility diagnostics are suppressed for truthful
`rustok_payment`, `rustok_order`, and `rustok_inventory` labels. The retained
cart diagnostic remains active.

## Preserved behavior

Compensation ordering, owner calls, lifecycle checks, journal transitions,
response types, payment behavior, inventory release behavior, and cart release
behavior are unchanged.

Inventory compensation is now also adapted. Cart compensation consumer mapping
and compile/runtime evidence remain open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, order/database scenarios,
restart scenarios, remote-port scenarios, workflows, or CI were executed.

Suggested maintainer checks:

```bash
node scripts/verify/verify-commerce-checkout-order-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-payment-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-compensation-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
cargo check -p rustok-commerce --lib
```
