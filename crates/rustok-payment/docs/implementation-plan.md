# `rustok-payment` implementation planning

The ecommerce-family implementation plan is maintained in:

`crates/rustok-commerce/docs/implementation-plan.md#payment-workstream`

This file intentionally contains no task checklist, completion marks, verification
status, execution order, or promotion decision. Those belong to the main ecommerce
plan because `rustok-payment` is an owner module inside the ecommerce family, not
the family orchestration root.

Payment-specific behavioral and operational documentation remains in:

- `crates/rustok-payment/docs/provider-webhooks.md`
- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`

The payment storefront boundary guard remains:

`npm run verify:payment:storefront-boundary`

Any completed or newly discovered payment task must update the payment workstream
in the main commerce implementation plan in the same commit.
