# Order checkout compensation local context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for locally produced outcomes from
`CheckoutOrderCompensationPort` after write admission, tenant parsing, actor parsing, and checkout
causation validation have succeeded.

The preceding order-owner slice introduced a context-preserving wrapper for both order checkout
write ports. The compensation wrapper now has a second public layer that retains the accepted
`PortContext`, delegates through that existing owner-context layer, and classifies only the stable
local compensation envelopes returned by the unchanged legacy implementation.

## Public API and compatibility

The crate-root API retains:

- `CheckoutOrderCompensationPort`;
- `CheckoutOrderCompensationRequest`;
- `CheckoutOrderCompensationSnapshot`;
- `InProcessCheckoutOrderCompensationPort`;
- `in_process_checkout_order_compensation_port`;
- `InProcessCheckoutOrderCompensationPort::new`;
- `InProcessCheckoutOrderCompensationPort::with_identity_port`.

The public `checkout_owner_context` module path is retained as a compatibility facade. It exports:

- the new compensation local-context wrapper and factory;
- the existing payment-settlement context wrapper and factory.

The original `checkout_owner_context.rs` implementation is compiled as the private
`checkout_owner_context_impl` module. External callers therefore cannot select the admission-only
compensation wrapper while existing crate-root and module-path names remain available.

## Layering and ordering

The public compensation call flows through these layers:

1. retain a clone of the delegated context for post-call diagnostics;
2. call the existing order owner-context wrapper;
3. require write policy;
4. require write semantics;
5. parse tenant UUID;
6. parse actor UUID;
7. require checkout causation identity;
8. delegate the original context and request to the unchanged compensation owner implementation;
9. classify a returned error only when its exact code and message identify a covered local outcome;
10. return the same `PortError` unchanged.

Admission and delegated context-validation failures are emitted by the inner owner-context wrapper
and return before the legacy owner call. They do not match the local compensation envelope set and
are not duplicated by the outer layer.

## Covered local outcomes

The outer wrapper classifies four exact existing `code + message` pairs:

| Stable code | Stable message | Local operation |
| --- | --- | --- |
| `order.checkout_compensation_identity_invalid` | `checkout compensation request is invalid` | `validate_request` |
| `order.checkout_compensation_identity_conflict` | `checkout order identity conflicts with the compensation request` | `validate_durable_checkout_identity` |
| `order.checkout_compensation_state_conflict` | `checkout order changed while compensation was being applied` | `adopt_cancelled_after_transition_race` |
| `order.checkout_compensation_manual_reconciliation` | `checkout requires manual reconciliation` | `require_manual_reconciliation` |

Request rejection and the cancellation-race conflict use warning severity.

Durable identity conflict and manual reconciliation use error severity because they represent
owner-integrity or operator-action outcomes.

## Manual reconciliation attribution

Several existing owner branches intentionally publish the same reconciliation envelope:

- an expected order exists but no durable checkout identity can be resolved;
- the order is already paid, shipped, or delivered and cannot be cancelled automatically;
- the order lifecycle is unknown.

The public code and message do not distinguish those branches. The wrapper therefore uses the
truthful shared local operation `require_manual_reconciliation` rather than inventing a more specific
public classification.

The unchanged legacy `manual_reconciliation` helper continues to record the optional order id,
typed lifecycle state, exact internal reason, operation label, and owner-specific identifiers.

## Exact classification boundary

Classification uses both stable code and stable message.

This matters for `order.checkout_compensation_state_conflict`. The cancellation-race branch uses:

- `checkout order changed while compensation was being applied`.

The existing `OrderError::InvalidTransition` service mapper uses the same code with the distinct
message:

- `checkout order lifecycle conflicts with compensation`.

The service-transition envelope does not match the local cancellation-race pair and passes through
without being relabelled. All other identity-port, order-service, admission, context-validation, and
owner errors also pass through unchanged and without an additional local event.

## Diagnostic contract

Each covered local event records:

- truthful owner `rustok_order.checkout_compensation`;
- exact public operation `compensate_checkout_order`;
- exact local operation;
- boundary `checkout_order_compensation_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable code and message;
- typed error kind and retryability;
- the mapped `PortError` itself.

The wrapper does not construct a replacement error. It returns the exact `PortError` produced by the
inner owner implementation.

## Preserved behavior

This slice does not change:

- request, response, trait, constructor, or factory signatures;
- write admission or delegated context-validation behavior;
- checkout identity read or legacy adoption;
- the no-identity `Ok(None)` path when no expected order is recorded;
- request identity acceptance rules;
- durable identity comparison rules;
- order loading;
- pending/confirmed cancellation;
- already-cancelled idempotent adoption;
- transition-race reread and cancelled adoption;
- paid, shipped, delivered, or unknown reconciliation decisions;
- order-service calls;
- `OrderError` mapping;
- public codes, messages, kinds, or retryability.

## Static evidence

`scripts/verify/verify-order-compensation-local-context.mjs` guards:

- root and module-path compatibility cutover;
- context retention before owner delegation;
- exact code-and-message classification for all four local outcomes;
- exact local-operation attribution;
- integrity-versus-ordinary severity;
- full delegated context and stable boundary;
- same-error return and absence of replacement `PortError` construction;
- unmatched error passthrough;
- separation of cancellation-race and service-transition envelopes;
- preserved reconciliation reasons and cancellation-race evidence;
- preserved service error mappings.

`scripts/verify/verify-order-checkout-owner-context.mjs` is synchronized with the layered facade and
continues to guard admission, tenant, actor, causation, constructor, factory, and legacy delegation
invariants.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- inventory reservation owner admission and validation;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and the shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-compensation-local-context.mjs
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-order-checkout-compensation-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
