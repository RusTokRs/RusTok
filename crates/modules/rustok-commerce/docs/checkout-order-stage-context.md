# Checkout order stage error safety

Status: **source-reviewed / unvalidated**

## Scope

Staged checkout keeps three in-process Order owner call sites, and all three
project owner failures onto a bounded Commerce vocabulary:

- `CheckoutOrderStageExecutor::load_payment_ready_state` reads the owner
  projection through `CheckoutOrderRecoveryAdapter::read_checkout_order`, so the
  read is policy-checked and arrives as a bounded `PortError`;
- `CheckoutOrderCreationExecutor` creates the order, and re-reads it after an
  identity conflict;
- `CheckoutOrderConfirmationExecutor` validates the projection and confirms the
  pending order.

The shared projection lives in `checkout_order_owner_failure.rs`. It is total
over `OrderError`, so a new owner variant cannot quietly start leaking text
through a transparent conversion.

The staged-checkout journal persists `pipeline_failure_message`, never the raw
owner `Display`.

## Retired sources

These must not return; the guards fail when the files reappear:

- `checkout_order_stages_legacy.rs` — the order-stage compatibility include
  source, superseded by the mounted facade;
- `checkout_stage_pipeline.rs` — a dead duplicate of
  `checkout_stage_pipeline_owner_ports.rs`, which is the mounted module;
- Commerce-side `CheckoutCompletionPort` writes — the owner trait remains in
  `rustok-order`, but Commerce no longer drives it.

## Public and persisted error policy

| Owner failure | Commerce code | Commerce message | Retryable |
| --- | --- | --- | --- |
| `OrderError::Validation` | `checkout.order_request_invalid` | `Checkout order request is invalid` | no |
| `OrderNotFound` / `OrderReturnNotFound` / `OrderChangeNotFound` | `checkout.order_resource_not_found` | `Checkout order resource was not found` | no |
| `InvalidTransition` / `IdempotencyConflict` | `checkout.order_state_conflict` | `Checkout order state conflicts with the requested operation` | no |
| `OrderError::Database` | `checkout.order_temporarily_unavailable` | `Checkout order service is temporarily unavailable` | yes |
| `OrderError::CommandReceiptCorrupt` | `checkout.order_receipt_corrupt` | `Checkout order operation could not be completed safely` | no |
| `OrderError::Core` | `checkout.order_operation_failed` | `Checkout order operation could not be completed safely` | no |
| order-owned `PortError` | owner code, preserved | static kind-based message | owner value |

`CheckoutOrderStageError::bounded_failure()` exposes the code and retryability
to the mounted pipeline:

- `pipeline_error_code` returns the bounded code and falls back to
  `checkout.order_stage_failed`;
- `pipeline_failure_disposition` keeps a retryable order failure retryable
  instead of escalating it to compensation;
- `pipeline_failure_message` persists the bounded stage text, and falls back to a
  static sentence that names the pipeline stage for every owner, database, or
  event payload that has no bounded text of its own. The literal fallback reads
  `checkout pipeline failed at stage` followed by the stage label, so journal
  readers can still locate the failing stage without any third-party text.

## Diagnostics

Commerce emits only bounded facts:

- a diagnostic token whose `Debug` output is `redacted`;
- truthful owner `rustok_order` and the exact owner operation;
- tenant, operation, and order identity shapes;
- the full bounded port context projection for port calls: actor kind, claim and
  role counts, channel, locale, correlation, causation, trace, idempotency, and
  deadline facts;
- the failure kind label, exact owner code, owner-message shape and character
  length, and owner retryability;
- boundary `commerce_checkout_order_owner_boundary`.

Unavailable, timeout, invariant, corrupt-receipt, and core failures use error
severity. Validation, not-found, conflict, and forbidden failures use warning
severity.

## Preserved behavior

This slice does not change:

- the `inventory_reserved -> order_created -> payment_ready` checkpoint order;
- immutable order plan persistence, legacy snapshot/request hashing, or
  adoption semantics;
- the completion idempotency identity or the recovery-before-completion order;
- typed `OrderStatusKind` admission and identity validation;
- successful DTOs, HTTP, GraphQL, or native contracts;
- Commerce FFA or FBA status.

## Evidence

- `crates/modules/rustok-commerce/contracts/evidence/checkout-order-stage-error-safety-source-review.json`
- `scripts/verify/verify-commerce-checkout-order-stage-context.mjs`
- `scripts/verify/verify-commerce-checkout-completion-cutover.mjs`
- `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`

## Remaining work

- `StagedCheckoutError` still forwards the pipeline `Display` to its caller. The
  HTTP and GraphQL boundaries remap it, but a typed transport envelope would
  remove the last third-party text from the transport layer.
- The GraphQL order boundary classifies `CommandReceiptCorrupt` as retryable and
  temporarily unavailable, while this stage treats it as an invariant failure
  that needs operator review. Both mappers should agree.
- Compile, runtime, replay, restart, and database evidence remain open.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-order-stage-context.mjs
node scripts/verify/verify-commerce-checkout-completion-cutover.mjs
node --test scripts/verify/verify-commerce-checkout-completion-cutover.test.mjs
node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs
node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs
cargo check -p rustok-commerce --lib
```
