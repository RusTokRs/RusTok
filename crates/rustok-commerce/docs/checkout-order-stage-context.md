# Checkout order stage owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for the mounted
checkout order stage in
`crates/rustok-commerce/src/services/checkout_order_stages.rs`.

The mounted stage already used `CheckoutOrderRecoveryAdapter` and
`CheckoutCompletionPort`. Before this slice, three failed owner calls mapped a
`PortError` through the generic commerce boundary mapper with only a stage name:

- legacy/existing order recovery through `recover_existing_checkout`;
- owner completion through `complete_checkout`;
- order projection read through `read_checkout_order`.

The complete `PortContext` was therefore unavailable when those failures were
mapped, so correlation, actor, locale, causation, idempotency, deadline, typed
severity, and truthful owner-operation attribution were lost.

## Delivered source contract

The inventory-reserved stage retains one `write_context`:

- recovery receives `write_context.clone()`;
- completion receives `write_context.clone()` when recovery returns no order;
- both failure mappers retain the original context.

The projection reader retains one separate `read_context`:

- `read_checkout_order` receives `read_context.clone()`;
- the original read context remains available for failure mapping.

All three failures are attributed to owner `rustok_order` with exact operations:

- `recover_existing_checkout` and commerce stage `recover_existing`;
- `complete_checkout` and commerce stage `complete`;
- `read_checkout_order` and commerce stage `read_order`.

Before the existing public boundary mapping runs, the order-stage mapper records:

- the original `PortError`;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- owner, exact owner operation, and commerce stage;
- original code, public-safe message, typed kind, and retryability;
- boundary `commerce_checkout_order_stage`.

Unavailable, timeout, and invariant failures use error severity. Other owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- operation or plan journal reads and writes;
- immutable order plan persistence;
- inventory reservation execution;
- legacy snapshot and request hash calculation;
- recovery request contents or legacy adoption semantics;
- the shared completion request and its immutable reuse;
- completion idempotency key `checkout:{operation_id}:order:complete`;
- read contexts remaining non-idempotent;
- completion-result/order-projection identity validation;
- confirmed-order lifecycle validation before inventory adoption;
- inventory adoption and order-created/payment-ready checkpoints;
- projection locale and fallback-locale request values;
- payment-ready recovery lifecycle validation;
- `CheckoutOrderStageError::Boundary` stage, code, message, or retryability;
- FBA or FFA status.

Order-stage port errors continue through the same generic `Boundary` envelope
after diagnostics.

## Static evidence

`scripts/verify/verify-commerce-checkout-order-stage-context.mjs` guards:

- one retained write context, two delegation clones, and two mapper inputs;
- one retained read context, one delegation clone, and one mapper input;
- truthful order owner and exact recovery/completion/read operations;
- existing actor, locale fallback, correlation, causation, idempotency, and
  deadline construction;
- full available `PortContext` fields in diagnostics;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before unchanged public boundary mapping;
- unchanged boundary envelope fields;
- unchanged legacy hashes, request reuse, projection validation, adoption, and
  stage checkpoints;
- absence of old context-dropping mapper calls and moved/inline contexts.

The existing completion-cutover verifier was synchronized only to require
`write_context.clone()` for the completion command and to forbid the old moved
context form. Its owner-boundary and cutover assertions remain intact.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- remaining fulfillment consumers and order adapters;
- remaining inventory, customer, tax, promotion, and ecommerce adapters;
- remaining payment execution or compensation adapters outside the mounted stages;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-order-stage-context.mjs
node scripts/verify/verify-commerce-checkout-completion-cutover.mjs
node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
