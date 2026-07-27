# Order checkout payment settlement local context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for locally produced outcomes from
`CheckoutOrderPaymentSettlementPort` after admission, tenant parsing, actor parsing, and checkout
causation validation have succeeded.

The preceding order-owner slice introduced the public context-preserving wrapper and retained:

- write-policy rejection context;
- write-semantics rejection context;
- tenant UUID parse context;
- actor UUID parse context;
- checkout causation identity context.

The legacy settlement implementation already retained owner-specific identifiers in partial logs,
but its local `PortError` outcomes returned through the wrapper without one complete event carrying
the delegated `PortContext`, exact public operation, exact local operation, and stable boundary.

This slice changes only that post-delegation local boundary.

## Covered local outcomes

The public wrapper classifies five exact existing `code + message` pairs:

| Stable code | Stable message | Local operation |
| --- | --- | --- |
| `order.checkout_payment_request_invalid` | `checkout payment settlement request is invalid` | `validate_request` |
| `order.checkout_payment_identity_missing` | `checkout requires manual reconciliation` | `require_durable_checkout_identity` |
| `order.checkout_payment_identity_conflict` | `checkout order identity conflicts with the payment settlement request` | `validate_durable_checkout_identity` |
| `order.checkout_payment_state_conflict` | `checkout order lifecycle does not allow payment settlement` | `validate_payment_settlement_lifecycle` |
| `order.checkout_payment_reference_conflict` | `checkout order is settled by another payment identity` | `validate_settled_payment_identity` |

The wrapper does not construct a replacement error. It emits structured diagnostics and returns the
same `PortError` unchanged.

## Exact classification boundary

Classification uses both the stable code and the stable public message.

This is important for `order.checkout_payment_state_conflict`, which is also used by the existing
`OrderError::InvalidTransition` service mapper with the distinct public message
`order lifecycle conflicts with payment settlement`.

The service-transition envelope does not match the local lifecycle pair and therefore passes
through without being relabelled as a local validation outcome. All other unmatched settlement,
identity-port, order-service, admission, or context-validation errors also pass through unchanged
and without an additional local event.

## Diagnostic contract

Each covered event records:

- truthful owner `rustok_order.checkout_payment_settlement`;
- exact public operation `settle_checkout_payment`;
- exact local operation;
- boundary `checkout_order_payment_settlement_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable code and message;
- typed error kind and retryability;
- the mapped `PortError` itself.

Request rejection and ordinary lifecycle conflict use warning severity.

Missing durable identity, durable identity conflict, and settled payment identity conflict use error
severity because they represent owner-integrity or manual-reconciliation outcomes. Their existing
legacy owner logs remain unchanged and retain the detailed checkout, cart, order, collection, and
payment identity evidence.

## Preserved behavior

This slice does not change:

- trait, request, response, constructor, or factory signatures;
- write admission or delegated context-validation ordering;
- checkout identity read or legacy adoption;
- request acceptance rules;
- durable identity comparison rules;
- order loading or locale fallback;
- confirmed-to-paid transition behavior;
- already-paid, shipped, or delivered idempotent adoption;
- pending, cancelled, or unknown lifecycle rejection;
- payment reference or method comparison;
- order-service calls;
- `OrderError` mapping;
- public codes, messages, kinds, or retryability.

The original context and request are delegated to the existing owner implementation. A clone of the
accepted context is retained only for diagnostics after an error is returned.

## Static evidence

`scripts/verify/verify-order-payment-settlement-local-context.mjs` guards:

- context retention before owner delegation;
- post-delegation mapping of the same returned `PortError`;
- exact code-and-message classification for all five local outcomes;
- exact local-operation attribution;
- integrity-versus-ordinary severity;
- full delegated context and stable boundary;
- absence of replacement `PortError` construction;
- passthrough of unmatched errors;
- separation of the local lifecycle envelope from the service-transition envelope;
- preservation of all five legacy codes and messages;
- preservation of mark-paid `OrderError` mapping.

The preceding `verify-order-checkout-owner-context.mjs` remains applicable because admission,
context validation, public factories, constructors, and delegation ordering are preserved.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- compensation request, identity, cancellation-race, and reconciliation local outcomes;
- inventory reservation owner admission and validation;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and the shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-payment-settlement-local-context.mjs
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-order-payment-settlement-error-context.mjs
node scripts/verify/verify-order-payment-settlement-typed-status.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
