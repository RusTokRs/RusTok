# Order checkout owner admission and context validation

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for the two checkout write ports
owned by `rustok-order`:

- `CheckoutOrderPaymentSettlementPort`;
- `CheckoutOrderCompensationPort`.

The existing owner implementations already enforced write admission, tenant UUID parsing, actor
UUID parsing, and checkout causation matching. Those checks used context-dropping `?` admission
forms and partial validation warnings.

This slice introduces public wrapper ports that perform the same checks before delegating to the
existing implementations.

## Public API cutover

The crate-root API retains the existing names:

- `CheckoutOrderPaymentSettlementPort`;
- `SettleCheckoutOrderPaymentRequest`;
- `InProcessCheckoutOrderPaymentSettlementPort`;
- `in_process_checkout_order_payment_settlement_port`;
- `CheckoutOrderCompensationPort`;
- `CheckoutOrderCompensationRequest`;
- `CheckoutOrderCompensationSnapshot`;
- `InProcessCheckoutOrderCompensationPort`;
- `in_process_checkout_order_compensation_port`.

The wrapper structs retain `new` and `with_identity_port` constructor parity. Existing commerce
consumers continue to import the same crate-root names.

The legacy implementation modules are now crate-private. This prevents an external caller from
bypassing the context-preserving wrapper through the old module-scoped factories while preserving
the root contracts used by the repository.

## Admission contract

Both public wrappers preserve the existing order:

1. require write policy;
2. require write semantics;
3. parse tenant UUID;
4. parse actor UUID;
5. require checkout causation identity;
6. delegate to the existing owner implementation.

Policy and write-semantics rejections retain the original `PortError` unchanged.

Diagnostics record:

- truthful owner;
- exact public operation;
- phase `policy` or `write_semantics`;
- boundary;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- original code, message, typed kind, and retryability;
- the original `PortError`.

Unavailable, timeout, and invariant failures use error severity. Ordinary rejection uses warning
severity.

## Context validation contract

Tenant and actor parsing preserve the existing stable public envelopes:

- `order.tenant_id_invalid` / `order request context is invalid`;
- `order.actor_id_invalid` / `order request context is invalid`.

The original UUID parse cause remains internal structured evidence.

Causation matching preserves the existing owner-specific envelopes:

- settlement: `order.checkout_payment_causation_invalid`;
- compensation: `order.checkout_compensation_causation_invalid`;
- message: `checkout operation context is invalid`.

Missing, malformed, or mismatched causation remains rejected. The expected operation UUID and raw
delegated causation value remain internal diagnostics.

Each context-validation event records the full available `PortContext`, exact owner, exact
operation, validation phase, boundary, mapped code/message/kind/retryability, and mapped
`PortError`. The same constructed error is returned unchanged.

## Preserved owner behavior

After wrapper validation succeeds, the original settlement and compensation implementations are
called with the original context and request.

This slice does not change:

- request or response DTOs;
- checkout identity read or legacy adoption;
- payment settlement request validation;
- durable identity missing/conflict handling;
- order lifecycle settlement rules;
- payment reference and method conflict rules;
- compensation identity validation;
- cancellation race adoption;
- manual reconciliation decisions;
- order service calls or `OrderError` mapping;
- public codes, messages, kinds, or retryability for downstream owner outcomes.

The original implementation repeats admission and context validation after wrapper success. Those
checks are deterministic and succeed for the already accepted context; downstream behavior remains
unchanged.

## Static evidence

`scripts/verify/verify-order-checkout-owner-context.mjs` guards:

- crate-private legacy modules and context-preserving root exports;
- constructor and factory compatibility;
- admission-before-tenant-before-actor-before-causation-before-delegation ordering;
- full admission context and technical-versus-ordinary severity;
- original admission error return;
- tenant/actor parse-cause retention;
- owner-specific causation codes;
- full context-validation diagnostics;
- stable public validation envelopes;
- preserved settlement and compensation owner behavior.

The existing settlement and compensation error-context verifiers remain applicable because the
legacy owner implementations and all downstream mappings are unchanged.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- settlement request, durable identity, lifecycle, and payment-reference local outcomes;
- compensation request, identity, cancellation-race, and reconciliation local outcomes;
- inventory reservation owner admission and validation;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-order-payment-settlement-error-context.mjs
node scripts/verify/verify-order-checkout-compensation-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
