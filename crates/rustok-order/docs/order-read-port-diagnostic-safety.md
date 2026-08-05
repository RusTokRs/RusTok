# Order read port diagnostic safety

Status: `source_reviewed_unvalidated`

This continuation closes the correlation-safe diagnostic gap in
`crates/rustok-order/src/order_read.rs`.

The reviewed boundary contains six owner read operations:

- order detail;
- order list;
- return detail;
- return list;
- order-change detail;
- order-change list.

The broad ecommerce mapper-cleanup item remains open.

## Previous exposure

The tenant parser and shared `OrderError` mapper retained stable public
`PortError` envelopes, but their logs could include request-owned or internal
payload:

- the complete tenant identifier;
- the complete actor context;
- request order, return, change, and customer UUID values;
- complete validation and transition fields through `OrderError` debug output;
- database and core payloads through complete `OrderError` debug output;
- UUID parser diagnostics.

These values are not required to classify an owner read failure.

## Source change

The boundary now defines one bounded context shape, one bounded request shape,
and one closed seven-variant owner-error shape.

Context diagnostics retain:

- correlation identifier and static owner operation;
- tenant and actor identifier lengths;
- actor kind;
- claim and role counts;
- channel presence and length;
- locale length;
- causation, traceparent, and idempotency-key presence and lengths;
- deadline.

Request diagnostics retain only:

- presence and non-nil shape for order, return, change, and customer UUIDs;
- status, change-type, and fallback-locale lengths.

Owner-error diagnostics retain only:

- the closed variant name;
- aggregate text-field count and total length;
- aggregate UUID-field count and non-nil count;
- whether an opaque database or core payload was present.

No complete `OrderError`, parser error, actor, tenant, channel, or resource UUID
value is logged by the reviewed parser or mapper.

## Preserved contracts

All six owner service calls, input forwarding, pagination, filtering, locale
fallback, totals, and projections remain unchanged.

The exact public mappings remain:

- validation -> `order.validation`, validation, non-retryable;
- order not found -> `order.order_not_found`, not found, non-retryable;
- return not found -> `order.return_not_found`, not found, non-retryable;
- change not found -> `order.change_not_found`, not found, non-retryable;
- invalid transition -> `order.invalid_transition`, conflict, non-retryable;
- database -> `order.database_unavailable`, unavailable, retryable;
- core -> `order.operation_failed`, invariant violation, non-retryable.

The tenant parser still returns `order.context_invalid` with validation kind and
`order request context is invalid`.

Database and core failures remain error-level diagnostics. Validation,
not-found, and lifecycle conflicts remain warning-level diagnostics.

## Deliberately open

This slice does not change:

- checkout recovery identity, hash/serde, read-not-found, or lifecycle logs;
- other Order adapters and public HTTP/GraphQL envelopes;
- payment execution or compensation;
- fulfillment, inventory, tax, promotion, or remaining ecommerce adapters;
- FFA/FBA status.

The master implementation-plan item therefore remains open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were
executed. The accompanying verifier is retained as a source contract and was
not run. No compile or runtime status is promoted from this review.
