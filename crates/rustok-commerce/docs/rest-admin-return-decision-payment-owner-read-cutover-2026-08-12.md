# Commerce REST admin return-decision Payment owner-read cutover

Status: `source_complete_validation_pending`

Date: 2026-08-12

## Scope

The mounted `POST /admin/orders/{id}/returns/decision` refund branch no longer constructs `PaymentService` to discover the captured collection when the request omits `payment_collection_id`.

`ReturnDecisionOwnerOrchestrationService` now receives the host-selected `PaymentAdminReadPort` from `CommerceHttpRuntime` and calls `list_payment_collection_projections` with the legacy lookup shape: page 1, one item, `status = captured`, and the current order id. An explicitly supplied payment collection id still bypasses the lookup exactly as before.

The Payment read context is derived from the authenticated return-decision root `PortContext`, retains tenant, actor, locale, channel and deadline, receives an operation-bound correlation id, and clears write-only idempotency metadata before the read call.

Payment read failures cross the mounted REST boundary as bounded `PortError` diagnostics. Raw owner messages are not logged by the new mapper.

## Deliberate boundary

Refund creation/provider execution remains on the existing `PaymentOrchestrationService` path so this slice does not change provider journal keys, reservation/reconciliation behavior, provider registry selection, refund DTOs, or public success responses.

Mounted GraphQL `createOrderReturnDecision`, broader return completion, legacy unmounted compatibility handlers, and the broad ecommerce topology P0 remain separate work. This change does not promote ecommerce FBA/FFA status.

## Validation

Pending GitHub Actions execution for the branch/PR. The focused source verifier is `scripts/verify/verify-commerce-rest-admin-return-decision-order-owner-port-cutover.mjs`; the ecommerce hardening workflow also runs the focused Rust check for the ecommerce owner/transport crates.
