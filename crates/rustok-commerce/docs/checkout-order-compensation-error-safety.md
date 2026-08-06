# Checkout order compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted Commerce checkout compensation facade now adapts both payment and
order owner ports while retaining
`checkout_compensation_owner_ports.rs` unchanged as private business logic.

This slice closes the order consumer mapper only. Inventory and cart
compensation remain separate work.

## Order boundary policy

The facade wraps every mounted order composition path:

- the default in-process order compensation factory;
- the identity-aware in-process constructor;
- custom `CheckoutOrderCompensationPort` injection.

The wrapper delegates the original `PortContext` and request unchanged and
preserves the successful `CheckoutOrderCompensationSnapshot`,
`PortError.kind`, exact owner `code`, and `retryable`.

Before the retained Commerce mapper sees a failure, the owner message is
replaced with a static message selected by `PortErrorKind`. The
`order.checkout_compensation_manual_reconciliation` code receives
`Checkout order compensation requires manual reconciliation`.

Consequently, owner text cannot enter the public compensation error or the
retryable operation-journal message.

## Diagnostics

The order adapter records only:

- tenant, actor, channel, locale, correlation, causation, trace, and idempotency
  shapes;
- claim and role counts;
- checkout-operation/cart UUID validity;
- expected-order UUID shape;
- reason presence and length;
- owner code, kind, retryability;
- owner-message presence and length;
- a redacted diagnostic token.

It does not record the complete `PortError`, owner message text, raw context
values, order identifiers, or reason text.

The retained compatibility event is suppressed only for truthful
`rustok_order` and `rustok_payment` owner labels. Inventory and cart
compatibility diagnostics continue unchanged.

## Preserved behavior

The compensation source, ordering, owner calls, lifecycle checks, journal
transitions, response DTOs, payment policy, inventory release, and cart release
are unchanged.

## Remaining work

Inventory and cart compensation consumer mappers still use the retained generic
boundary and remain open. Compile and runtime evidence also remain open.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-order-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-payment-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, order/database scenarios,
restart scenarios, remote-port scenarios, workflows, or CI were executed.
