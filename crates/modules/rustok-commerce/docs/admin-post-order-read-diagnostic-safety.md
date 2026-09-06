# Admin post-order read diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the shared `map_admin_post_order_port_error` event used by the four mounted Commerce admin post-order read routes:

- return list;
- return detail;
- order-change list;
- order-change detail.

The routes continue to call the host-selected typed order read port with the same actor, locale, channel, deadline, filters, pagination, and resource identifiers. Only the failure diagnostic projection changes.

## Bounded diagnostic projection

The full `PortError` is replaced in the event by the stable marker `redacted`.

The event no longer records the complete correlation ID, tenant string, actor UUID, return UUID, change UUID, order UUID, `PortActor`, channel, or locale. Instead it records:

- correlation presence and length;
- a closed tenant UUID-text shape;
- closed actor and optional resource UUID shapes;
- channel presence and length;
- locale length;
- the configured deadline;
- the stable internal code and retryability;
- owner/consumer operations, error kind, public code, HTTP status, owner, boundary, and static event message.

Required UUID shapes are `nil` or `non_nil`. Optional resource shapes are `absent`, `present_nil`, or `present_non_nil`. Tenant text additionally fails closed to `empty` or `invalid` if it is not a UUID.

## Preserved behavior

This work does not change:

- `PortErrorKind` to HTTP policy mapping;
- public status, code, or message selection;
- `HttpError::new(status, code, message)` construction;
- route permissions;
- return and order-change owner operations;
- request filters and pagination;
- successful response bodies;
- the four existing mapper callsites.

## Remaining boundary

This slice does not close raw diagnostic payloads in Commerce admin return/change mutation controllers, the broader admin order controller, shipping, payment, fulfillment, product, or storefront transports. The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-post-order-read-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-post-order-read-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-post-order-read-cutover.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime status is promoted.
