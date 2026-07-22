# `rustok-payment` implementation planning

The ecommerce-family implementation plan is maintained in:

`crates/rustok-commerce/docs/implementation-plan.md#payment-workstream`

This file intentionally contains no task checklist, completion marks, verification
status, execution order, or promotion decision. Those belong to the main ecommerce
plan because `rustok-payment` is an owner module inside the ecommerce family, not
the family orchestration root.

Compatibility metadata for the generic module verifier follows. It mirrors the
main plan and must not be edited as an independent planning decision:

- FBA status: `boundary_ready`
- Registry: `payment-fba-registry.json`
- Additional workflow contract:
  `crates/rustok-payment/contracts/payment-checkout-compensation-v1.json`

Payment-specific behavioral and operational documentation remains in:

- `crates/rustok-payment/docs/provider-webhooks.md`
- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`
- `crates/rustok-payment/contracts/payment-checkout-compensation-v1.json`

Checkout compensation now enters payment through
`CheckoutPaymentCompensationPort`. Provider journal reads, canonical cancel replay,
external provider cancellation, local collection cancellation, and uncertain
outcome classification remain inside `rustok-payment`. The mounted commerce
compensation source receives only a safe `PaymentCollectionStatusSnapshot` and no
longer constructs `PaymentService`, `PaymentProviderOperationJournal`, or
`PaymentOrchestrationService`.

The payment storefront owner transport continues to use
`execute_selected_transport` for `create_payment_collection`,
`fetch_payment_collection`, and `fetch_refund_summary`. Completion and verification
status for that boundary is maintained only in the payment workstream of the main
commerce plan.

The payment storefront boundary guard remains:

`npm run verify:payment:storefront-boundary`

The checkout compensation owner-boundary guard is:

`node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`

Any completed or newly discovered payment task must update the payment workstream
in the main commerce implementation plan in the same commit.
