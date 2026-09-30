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

The logger no longer serializes the typed error at all. It records only the bounded `*_state` identity facts plus static route operation, owner, source owner, error kind, public code, HTTP status, boundary, and the existing static log message.

## Preserved behavior

This work does not change:

- permission checks or route inputs;
- checkout state-machine semantics, operation-journal claim/lease behavior, or response mapping;
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

- `crates/modules/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-checkout-operation-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime
status is promoted.


## Checkout compensation boundary update — 2026-09-30

The active Admin compensation path now uses host-composed typed owner ports instead of constructing
Payment/Order/Inventory/Cart owners directly inside Commerce. The HTTP compensation and compensation-sweep
writes require a caller-owned `Idempotency-Key`; the sweep derives a bounded per-operation internal key from
that caller identity plus the operation UUID.

The Commerce compensation service no longer owns `PaymentService`, `OrderService`,
`PaymentProviderOperationJournal`, or checkout-order identity implementations. Payment and Order cancellation
remain inside their owner ports, while Commerce retains only orchestration state/journal coordination and the
Cart/Inventory typed boundaries.

This remains **source-ready / unvalidated**: no local build/test or verifier execution is claimed.
