# Payment checkout compensation local and admission context

Status: **source-ready / unvalidated**

## Scope

This slice closes correlation-safe local context retention for the canonical payment compensation owner boundary:

- `CheckoutPaymentCompensationPort::compensate_checkout_payment`;
- root `InProcessCheckoutPaymentCompensationPort`;
- root `in_process_checkout_payment_compensation_port`;
- the compatibility namespace `rustok_payment::checkout_compensation::*`.

The persistent state machine in `checkout_compensation.rs` remains behaviorally unchanged. It is now loaded as a private module. Both the crate root and the public `checkout_compensation` namespace expose the context wrapper, so callers cannot bypass safe local diagnostics by choosing the legacy module path.

## Public module facade

`lib.rs` loads the sources as separate layers:

- `checkout_compensation.rs` becomes private `checkout_compensation_persistent`;
- `checkout_compensation_context.rs` owns the public wrapper implementation;
- `checkout_compensation_api.rs` is the public module facade.

The facade preserves the existing public names:

- `CheckoutPaymentCompensationPort`;
- `CheckoutPaymentCompensationRequest`;
- `InProcessCheckoutPaymentCompensationPort`;
- `in_process_checkout_payment_compensation_port`.

The trait and request continue to be the original owner contracts. The public type and factory now always resolve to the context wrapper. The persistent implementation type and factory are not publicly re-exported.

## Delegation order

The wrapper performs no new lifecycle or provider policy. Its source order is:

1. clone the incoming `PortContext` for diagnostics;
2. retain safe request facts;
3. delegate the original context and request to the unchanged persistent owner;
4. inspect only a returned `PortError`;
5. classify only exact stable `code + message` pairs;
6. return the same `PortError` unchanged.

The persistent owner continues to own write policy, write semantics, tenant parsing, checkout-operation causation, optional-collection no-op behavior, identity validation, lifecycle admission, provider journal recovery, provider cancellation, local cancellation, and commit checkpointing.

## Admission outcomes

The public wrapper now retains full context for the two stable admission envelopes returned before compensation work begins:

| Stable envelope | Local operation | Severity |
| --- | --- | --- |
| `port.idempotency_key_required` / `write port calls require a non-empty idempotency key` | `admit_write_idempotency` | warning |
| `port.deadline_required` / `port calls require deadline semantics` | `admit_deadline` | error |

The timeout kind for a missing deadline is classified as technical. A missing idempotency key remains an ordinary validation rejection. No code, message, kind, retryability, or execution order changes.

Tenant parsing and checkout-operation causation errors remain pass-through. The persistent owner already emits their owner-local events; the wrapper deliberately avoids a duplicate local event for those outcomes.

## Retained request facts

Covered diagnostics retain:

- checkout operation id;
- optional payment collection id;
- optional compensation-reason character length;
- metadata JSON kind;
- object-field or array-element count when applicable.

The raw compensation reason and metadata value are never recorded. Metadata can contain arbitrary caller and provider data, and reason is unvalidated before owner normalization.

## Covered stable owner outcomes

The exact mapper also retains the existing local classification for:

- invalid compensation identity;
- collection/payment/refund not found;
- manual reconciliation;
- payment and compensation lifecycle conflicts;
- unsupported provider-journal state;
- invalid provider metadata or conflicting provider identity;
- provider request encoding failure;
- storage unavailability and owner validation;
- provider unavailable, rejected, invalid response, or missing configuration.

Unavailable, timeout, and invariant outcomes use error severity. Manual reconciliation and unsupported durable provider-journal state also use error severity. Ordinary validation, not-found, rejection, identity, lifecycle, and concurrent-state conflicts use warning severity.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_payment`;
- public operation `compensate_checkout_payment`;
- operation-specific local label;
- boundary `checkout_payment_compensation_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- safe request facts;
- exact stable code and message;
- typed error kind and retryability;
- the complete delegated `PortError`.

## Preserved behavior

This work does not change:

- compensation request or response DTOs;
- public codes, messages, kinds, or retryability;
- write policy, write semantics, tenant, or causation validation;
- optional missing-collection no-op behavior;
- captured-payment refund-policy reconciliation;
- payment collection lifecycle classification;
- provider selection or manual-provider fallback;
- canonical `payment_collection:{collection_id}:cancel` idempotency key;
- provider request metadata and `cancel_payment_collection` marker;
- provider journal begin, claim, replay, error, success, and reconciliation checkpoints;
- provider payment identity recovery;
- local cancellation race handling;
- final provider-operation commit checkpoint;
- commerce payment-before-order-before-inventory-before-cart compensation ordering;
- provider-registry constructor behavior.

## Static evidence

`scripts/verify/verify-payment-checkout-compensation-local-context.mjs` guards:

- private persistent source and public facade module wiring;
- root and module-path wrapper type/factory exports;
- absence of a public persistent type/factory bypass;
- wrapper constructor delegation for default and provider-registry construction;
- context and safe-fact retention before unchanged owner delegation;
- exact admission and owner-outcome classification;
- same delegated `PortError` return;
- length/shape-only payload evidence and absence of raw reason/metadata diagnostics;
- complete `PortContext`, request-fact, and typed error fields;
- tenant and causation pass-through;
- unchanged persistent policy, provider idempotency key, journal, cancellation, and checkpoint markers;
- mounted Commerce use of root contracts and wrapper construction.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- broader compensation tenant/causation diagnostic enrichment without duplicate events;
- direct payment GraphQL and HTTP query/mutation envelopes;
- remaining order, fulfillment, inventory, customer, tax, promotion, ecommerce adapter, and non-`PortError` envelopes;
- compile, provider replay, restart, remote-port, and cross-transport runtime evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-checkout-compensation-local-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
