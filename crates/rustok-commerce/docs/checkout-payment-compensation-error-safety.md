# Checkout payment compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted Commerce checkout compensation facade is
`checkout_compensation_payment_safe.rs`. It retains the existing
`checkout_compensation_owner_ports.rs` business logic privately and adapts the
canonical payment and order compensation ports before their failures reach the
legacy Commerce mapper.

This document records the payment side of that combined facade. The later
order slice reuses the same mount; inventory and cart compensation remain open.

## Payment boundary policy

The default payment factory, provider-registry constructor, and custom payment
port injection are all wrapped.

The wrapper preserves:

- the canonical request and response types;
- the complete `PortContext` delegated to the payment owner;
- `PortError.kind`;
- the exact owner `code`;
- `retryable`;
- successful compensation snapshots and ordering.

It replaces `PortError.message` with a Commerce-owned static message before the
retained mapper can construct `CheckoutCompensationError` or persist
`compensation.to_string()` through `mark_compensation_retryable`.

The manual-reconciliation owner code receives the static message
`Checkout payment compensation requires manual reconciliation`.

## Diagnostics

The facade emits a redacted diagnostic token plus bounded facts only:

- identity and context shapes;
- claim and role counts;
- request UUID presence/shape;
- reason presence and length;
- metadata kind and entry count;
- stable owner code, error kind, retryability;
- owner-message presence and length.

It does not record the complete `PortError`, owner message text, raw tenant,
actor, channel, locale, correlation, causation, trace, idempotency, reason, or
metadata values.

The retained compatibility diagnostic is suppressed when the truthful owner is
`rustok_payment`; the bounded facade event is authoritative.

## Preserved behavior

The retained compensation source remains unchanged. Payment, order, inventory,
and cart ordering, operation-journal transitions, replay behavior, DTOs, and
public service signatures are preserved.

Order compensation is now also adapted by the combined facade. Inventory and
cart compensation mapper cleanup remains open.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-payment-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-order-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, payment-provider calls,
database scenarios, restart scenarios, remote-port scenarios, workflows, or CI
were executed for this source review.
