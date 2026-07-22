# Checkout payment and fulfillment owner-stage cutover

Status: `source_ready_unvalidated`

Last reviewed: 2026-07-22

## Production mount

`crates/rustok-commerce/src/services/mod.rs` mounts:

`crates/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs`

The previous pipeline source remains unmounted compatibility source until compile,
replay, restart, and upgraded-path evidence is retained.

## Payment owner

- contract: `crates/rustok-payment/contracts/payment-checkout-execution-v1.json`
- port: `CheckoutPaymentExecutionPort`
- owner source: `crates/rustok-payment/src/checkout_execution.rs`
- operations: prepare, authorize, capture, read

Payment owner retains collection lifecycle, provider registry, provider operation
journal, canonical authorize/capture keys, provider result checkpointing, local
collection mutation, and reconciliation policy.

Canonical upgraded replay identities remain:

- `payment_collection:{collection_id}:authorize`
- `payment_collection:{collection_id}:capture`

Provider request payloads preserve the previous `authorize_payment_collection` and
`capture_payment_collection` orchestration values so existing journal rows are
adopted after upgrade.

## Fulfillment owner

- contract: `crates/rustok-fulfillment/contracts/fulfillment-checkout-execution-v1.json`
- port: `CheckoutFulfillmentExecutionPort`
- owner source: `crates/rustok-fulfillment/src/checkout_execution.rs`
- operations: ensure set, read set

Commerce maps immutable checkout cart-line plans to typed order-line commands.
Fulfillment owner creates or adopts stable fulfillment keys through
`FulfillmentService::list_by_order` and `create_fulfillment`. Commerce no longer
queries the `fulfillments` table.

Metadata identity remains an owner-local compatibility mechanism. A typed durable
fulfillment identity and uniqueness migration remain open. Duplicate identities
fail closed.

## Order payment settlement owner

- contract: `crates/rustok-order/contracts/order-checkout-payment-settlement-v1.json`
- port: `CheckoutOrderPaymentSettlementPort`
- owner source: `crates/rustok-order/src/checkout_payment_settlement.rs`
- operation: settle captured payment identity

Order owner resolves checkout identity, marks a confirmed order paid, and adopts
paid/shipped/delivered replay only when the payment reference and method match.
Commerce no longer constructs `OrderService` in the fulfillment stage.

## Commerce responsibility

Commerce retains:

- immutable order and fulfillment plan mapping;
- marketplace economics sequencing;
- `payment_ready -> payment_authorized -> payment_captured` checkpoints;
- `payment_captured -> fulfillment_created` checkpoint;
- final cart completion and operation completion.

Mounted payment, fulfillment, and pipeline source contains no `PaymentService`,
`FulfillmentService`, `OrderService`, `PaymentProviderOperationJournal`,
`PaymentOrchestrationService`, or raw fulfillment SQL.

## Static guard

`node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`

## Evidence still required

- Rust formatting and compile checks;
- prepare/reuse and identity-conflict collection scenarios;
- authorize/capture duplicate, provider-success/local-failure, unknown-outcome,
  process-exit, and restart scenarios;
- fulfillment create/adopt, duplicate identity, partial set, concurrent create,
  process-exit, and restart scenarios;
- order paid-settlement conflict and replay scenarios;
- clean/upgraded databases, mounted transports, remote adapters, and real provider
  evidence.

No FBA/FFA status promotion is claimed by this source change.
