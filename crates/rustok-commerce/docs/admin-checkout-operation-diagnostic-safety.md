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
transports, owner adapters, or remaining non-`PortError` envelopes. The broader ecommerce
correlation-safe mapper task remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-checkout-operation-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime
status is promoted.
