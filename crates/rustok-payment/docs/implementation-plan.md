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
- Additional workflow contracts:
  - `crates/rustok-payment/contracts/payment-checkout-compensation-v1.json`
  - `crates/rustok-payment/contracts/payment-checkout-execution-v1.json`

Payment-specific behavioral and operational documentation remains in:

- `crates/rustok-payment/docs/provider-webhooks.md`
- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`
- `crates/rustok-payment/contracts/payment-checkout-compensation-v1.json`
- `crates/rustok-payment/contracts/payment-checkout-execution-v1.json`

Checkout compensation enters payment through
`CheckoutPaymentCompensationPort`. Provider journal reads, canonical cancel replay,
external provider cancellation, local collection cancellation, and uncertain
outcome classification remain inside `rustok-payment`. The mounted commerce
compensation source receives only a safe `PaymentCollectionStatusSnapshot` and no
longer constructs `PaymentService`, `PaymentProviderOperationJournal`, or
`PaymentOrchestrationService`.

Staged checkout prepare, authorize, capture, and recovery reads enter payment
through `CheckoutPaymentExecutionPort`. Collection lifecycle, provider registry,
provider operation journal, CAS execution, provider-result checkpointing, local
collection mutation, and reconciliation policy remain inside `rustok-payment`.
The mounted commerce payment stage receives only normalized
`PaymentCollectionResponse` projections.

The execution owner preserves the pre-cutover provider identities and immutable
request payload values:

- `payment_collection:{collection_id}:authorize`
- `payment_collection:{collection_id}:capture`
- `authorize_payment_collection`
- `capture_payment_collection`

An upgraded retry therefore adopts the existing provider-operation journal row
instead of executing a second provider effect. Provider success is checkpointed
before local collection mutation; a local persistence failure after provider
success is classified as reconciliation-required.

The payment storefront owner transport continues to use
`execute_selected_transport` for `create_payment_collection`,
`fetch_payment_collection`, and `fetch_refund_summary`. Completion and verification
status for that boundary is maintained only in the payment workstream of the main
commerce plan.

Boundary guards:

- `npm run verify:payment:storefront-boundary`
- `node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`
- `node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`

Compile, provider replay, process-exit, restart, contention, mounted transport, and
remote-profile evidence remain unexecuted. No FBA/FFA status promotion is claimed.

Any completed or newly discovered payment task must update the payment workstream
in the main commerce implementation plan in the same commit.
