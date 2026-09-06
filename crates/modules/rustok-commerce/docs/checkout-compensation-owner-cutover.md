# Checkout compensation owner cutover

Status: `source_ready_unvalidated`

Last reviewed: 2026-07-22

## Production mount

`crates/rustok-commerce/src/services/mod.rs` mounts:

`crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs`

The previous `checkout_compensation.rs` file is not mounted and remains temporary
compatibility source until compile, replay, restart, and upgraded-path evidence is
retained.

## Owner boundaries

Order compensation uses:

- contract: `crates/rustok-order/contracts/order-checkout-compensation-v1.json`
- port: `CheckoutOrderCompensationPort`
- operation: `compensate_checkout_order`
- owner implementation: `crates/rustok-order/src/checkout_compensation.rs`

Payment compensation uses:

- contract: `crates/rustok-payment/contracts/payment-checkout-compensation-v1.json`
- port: `CheckoutPaymentCompensationPort`
- operation: `compensate_checkout_payment`
- owner implementation: `crates/rustok-payment/src/checkout_compensation.rs`

Commerce retains only orchestration-owned checkout operation state, reservation
mapping state, stage ordering, retry classification, and cart/inventory port calls.
The mounted compensation path does not construct `OrderService`, `PaymentService`,
`PaymentProviderOperationJournal`, or `PaymentOrchestrationService`.

## Execution order

1. Reject automatic compensation after `payment_captured`.
2. Ask payment owner to cancel or adopt the collection outcome.
3. Ask order owner to cancel or adopt the order outcome.
4. Release any pre-adoption inventory reservations still marked reserved.
5. Release the checkout cart.
6. Mark the commerce checkout operation compensated.

Captured payment, consumed inventory, paid/shipped/delivered order, unresolved
provider execution, and provider reconciliation states fail closed into manual
reconciliation.

## Upgraded replay compatibility

Payment owner preserves the pre-cutover provider cancellation identity:

`payment_collection:{collection_id}:cancel`

It also preserves the previous immutable provider request payload shape. An
upgraded retry therefore adopts the existing provider-operation journal row rather
than issuing a second provider cancellation.

Order owner resolves typed checkout identity first and permits legacy metadata
adoption only inside its temporary owner-local compatibility adapter.

## Static guard

`node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`

The guard rejects:

- remounting the previous compensation source;
- direct order/payment service construction in mounted commerce compensation;
- direct payment provider journal access in mounted commerce compensation;
- missing owner port calls, deadline, causation, or idempotency context.

## Evidence still required

- Rust formatting and compile checks;
- pending/authorized/cancelled/captured payment scenarios;
- pending/confirmed/cancelled/paid/shipped/delivered order scenarios;
- provider cancel replay using a pre-cutover journal row;
- provider unknown-outcome and reconciliation-required behavior;
- concurrent compensation claims and owner cancellation races;
- process exit after provider success and before local payment cancellation;
- process exit after owner cancellation and before commerce checkpoint;
- upgraded database, restart, mounted transport, and remote-adapter evidence.

No FBA/FFA status promotion is claimed by this source change.
