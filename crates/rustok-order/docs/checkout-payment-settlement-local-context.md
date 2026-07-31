# Order checkout payment settlement local context

Status: **source-ready / unvalidated**

## Scope

The payment-settlement local mapper is hosted in the shared
`checkout_owner_context.rs` wrapper. This update aligns that mapper with the safe
checkout context policy introduced while hardening Order compensation diagnostics.

`checkout_payment_settlement.rs`, its request/response contracts, identity handling,
lifecycle behavior, order-service calls, and public `PortError` mapping are unchanged.

## Stable-code attribution

The wrapper recognizes these stable codes:

- `order.checkout_payment_request_invalid` → `validate_request`;
- `order.checkout_payment_identity_missing` →
  `require_durable_checkout_identity`;
- `order.checkout_payment_identity_conflict` →
  `validate_durable_checkout_identity`;
- `order.checkout_payment_state_conflict` →
  `validate_payment_settlement_lifecycle`;
- `order.checkout_payment_reference_conflict` →
  `validate_settled_payment_identity`.

Human-readable `PortError.message` is not used as control flow. Unknown codes pass
through without an added local event. The same `PortError` is returned unchanged.

The shared state-conflict code can originate from local lifecycle validation or the
owner service transition mapper. Both are truthfully attributed to the stable
settlement lifecycle operation instead of relying on public wording.

## Safe context

The event retains correlation id and records only context shape:

- tenant and actor-id lengths;
- actor kind;
- claim and role counts;
- channel presence and length;
- locale length;
- causation, traceparent, and idempotency presence and lengths;
- deadline milliseconds.

Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are
not logged by the shared local mapper.

Request and ordinary lifecycle rejection remain warning severity. Missing identity,
durable identity conflict, and settled-payment identity conflict remain error
severity.

## Preserved behavior

This diagnostic-only update does not change:

- admission, tenant, actor, or causation ordering;
- checkout identity read or legacy adoption;
- request acceptance rules;
- durable identity comparisons;
- order loading or locale fallback;
- confirmed-to-paid transition;
- paid, shipped, or delivered replay adoption;
- lifecycle or payment-reference conflicts;
- public codes, messages, kinds, or retryability;
- Order FFA/FBA status.

Owner-local settlement diagnostics remain a separate cleanup.

## Static evidence

- `scripts/verify/verify-order-payment-settlement-local-context.mjs`
- `scripts/verify/verify-order-checkout-owner-context.mjs`
- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json`

No tests, verifiers, Cargo commands, formatting, workflows, or CI were executed by
the implementation agent.
