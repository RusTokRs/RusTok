# Admin checkout operation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the shared `admin_checkout_operation_http_error` event used by:

- admin checkout-operation lookup;
- explicit checkout compensation;
- compensation sweep storage failures.

The mapper continues to receive the typed source error and the full internal context required for
policy selection and identity adoption. Only the diagnostic projection is changed.

## Bounded diagnostic projection

Immediately before the `tracing::error!` event, the internal context is converted to
`AdminCheckoutOperationDiagnosticContext`.

Required tenant and actor UUIDs are represented only as `nil` or `non_nil`. Optional checkout,
reservation, payment, refund, order, return, and change UUIDs are represented only as `absent`,
`present_nil`, or `present_non_nil`.

The typed error is replaced in the event by the stable marker `redacted`. The event still records the
static route operation, owner, source owner, error kind, public code, HTTP status, boundary, and the
existing static log message.

## Preserved behavior

This work does not change:

- permission checks or route inputs;
- operation, compensation, and sweep service calls;
- typed policy matching;
- source-owner routing;
- not-found identity adoption;
- public status, code, or message selection;
- the single `HttpError::new(status, code, message)` constructor;
- successful operation and sweep response bodies.

The existing broad source verifier retains its original log-site markers. Those markers now point to
the bounded diagnostic context rather than the raw request/error context.

## Remaining boundary

This slice does not close raw diagnostic payloads in other Commerce admin controllers, storefront
transports, owner adapters, or remaining non-`PortError` envelopes.
The broader ecommerce correlation-safe mapper task remains open.

## Later change: checkout reconciliation action surface

The deep e-commerce audit (`docs/audits/ecommerce-deep-review-2026-10-07.md`) added two handlers to
the same controller: `POST /admin/checkout-operations/{id}/actions` and
`GET /admin/checkout-operations/{id}/actions`. They report failures through the same
`admin_checkout_operation_http_error` event via `map_reconciliation_error`, so the bounded projection,
the redacted error marker and the single `HttpError::new(status, code, message)` constructor are
reused unchanged. Their permission set is derived from the executed action
(`CheckoutReconciliationAction::required_permissions()`): `orders:manage` for the safe actions and
`orders:manage` + `payments:update` for `refund_full`, `refund_partial` and `void_authorization`. The
verifier expectation for the number of context-aware mapper callsites was updated from three to six
accordingly.

## Later change: bounded compensation retries and operational counters

The same change set bounds the compensation loop and makes parking observable.
`CheckoutOperationJournal::park_exhausted_compensation` moves an operation whose
`attempt_count` reached `MAX_CHECKOUT_COMPENSATION_ATTEMPTS` (8) from
`compensation_required`/`compensating` into `reconciliation_required` with the bounded code
`checkout.compensation_attempts_exhausted`; `POST /admin/checkout-operations/compensation-sweep`
reports those parks in the new `exhausted` field and reports a failed park with the bounded code
`checkout.compensation_exhaustion_park_failed` (logged with `error_kind`, never with raw error text).
Two bounded-cardinality Prometheus counters, `rustok_checkout_reconciliation_parked_total{reason}` and
`rustok_checkout_reconciliation_actions_total{action,result}`, count parks and operator actions; alert
rules are documented in `DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`
(§Operational signals). No tenant identifier is used as a label.

## Evidence

- `crates/modules/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-checkout-operation-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime
status is promoted.
