# Checkout compensation payment/order owner context

Status: **source-ready / unvalidated**

## Scope

This source work retains structured context for the mounted checkout compensation service's payment and
order owner calls in `crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs`.

The covered consumer operations are:

- payment compensation through `compensate_checkout_payment`;
- order compensation through `compensate_checkout_order`.

The payment owner now also retains stable local outcome context behind the canonical root payment
compensation construction. Inventory reservation release and cart read/release remain documented in
their separate compensation slices.

## Consumer-side delivered contract

The mounted service retains one context for each owner call:

- `payment_context` is cloned into `CheckoutPaymentCompensationPort` and retained for failure mapping;
- `order_context` is cloned into `CheckoutOrderCompensationPort` and retained for failure mapping.

A shared commerce diagnostic mapper receives the retained context plus:

- truthful payment owner `rustok_payment` or order owner `rustok_order`;
- exact owner operation `compensate_checkout_payment` or `compensate_checkout_order`;
- existing commerce stage `compensate_payment` or `compensate_order`;
- original `PortError`.

Before the existing manual-reconciliation or boundary mapping runs, the mapper emits one structured
event containing:

- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- owner, exact owner operation, and commerce stage;
- original error code, public-safe message, typed kind, and retryability;
- boundary `checkout_compensation_owner_port`.

Unavailable, timeout, and invariant failures use error severity. Other owner rejections use warning
severity.

## Payment owner local outcomes

`rustok-payment` now keeps the original public trait and request contract while exporting a context
wrapper under the canonical root names:

- root `InProcessCheckoutPaymentCompensationPort`;
- root `in_process_checkout_payment_compensation_port`.

The mounted commerce service already imports those root names, including its provider-registry
constructor path. It therefore uses the wrapper without changing commerce orchestration source.

The wrapper retains the delegated `PortContext`, checkout operation id, optional collection id, reason
length, and metadata shape/count. It does not log raw compensation reason or metadata. Exact stable
identity, collection, lifecycle, provider, storage, and reconciliation envelopes receive owner-local
diagnostics and return the same `PortError`.

The consumer event remains necessary because it records the commerce stage and the eventual commerce
boundary conversion. The owner-local event records the internal owner operation and safe request facts;
the two events have distinct boundaries and do not replace each other.

The complete owner contract is documented in
[`../../rustok-payment/docs/checkout-compensation-local-context.md`](../../rustok-payment/docs/checkout-compensation-local-context.md).

## Preserved behavior

This work does not change:

- checkout compensation claim, lease, retry, or journal behavior;
- captured-funds manual-reconciliation admission;
- payment-before-order-before-inventory-before-cart compensation ordering;
- payment or order request DTOs, reasons, metadata, or idempotency keys;
- typed payment collection and order cancelled-state validation;
- missing owner result reconciliation behavior;
- manual-reconciliation code recognition for payment and order owners;
- `CheckoutCompensationError::ManualReconciliation` contents;
- `CheckoutCompensationError::Boundary` stage, code, message, or retryability;
- inventory reservation release callsites or mapper;
- cart read/release callsites or mapper;
- FBA or FFA status.

The payment persistent owner keeps its canonical cancel provider key, journal recovery, provider
execution, local cancellation, and checkpoint ordering unchanged.

## Static evidence

`scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs` guards:

- one retained payment context and one delegation clone;
- one retained order context and one delegation clone;
- truthful owner and exact owner-operation attribution;
- full available `PortContext` fields;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before the unchanged manual-reconciliation/boundary routing;
- unchanged boundary envelope fields;
- unchanged payment/order lifecycle checks and compensation ordering;
- unchanged inventory/cart mapper paths;
- absence of the old inline context construction and context-dropping mapper calls.

`scripts/verify/verify-payment-checkout-compensation-local-context.mjs` separately guards canonical root
wrapper construction, safe request-fact retention, exact stable local outcomes, same error return, and
unchanged persistent provider/journal semantics.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- direct payment callers that bypass the canonical root wrapper through the legacy module path;
- payment owner policy, tenant, and causation diagnostics beyond the existing owner events;
- direct payment query/mutation transport envelopes;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` adapters;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

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
