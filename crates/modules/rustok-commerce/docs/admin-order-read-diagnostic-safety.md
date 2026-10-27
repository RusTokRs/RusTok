# Admin order read diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice covers the two mounted Commerce admin order read routes — the order list and the order
detail — as they are served today by `controllers/admin/orders_owner_ports.rs`.

The list reads `OrderReadPort` (`list_order_projections`); the detail reads three host-selected ports and
joins them: `OrderReadPort` (`read_order_projection`), `PaymentOrderReadPort`
(`find_latest_collection_by_order`), and `FulfillmentReadPort`
(`find_latest_fulfillment_by_order_projection`). All three requests are typed and
carry the same tenant default locale fallback.

The controller used to be duplicated in `controllers/admin/orders.rs`, a file that was never part of the
module tree: `controllers/admin/mod.rs` mounts `orders_owner_ports.rs` under the `orders` module name.
That file is deleted, so a verifier can no longer certify an unmounted copy of these routes.

## Bounded diagnostic projection

Reads log shape, never identity:

- the entry line records the operation plus `tenant_id_non_nil`, `actor_id_non_nil`, order presence and
  nil-shape, and the list filters as presence only (`status_filter_present`, `customer_filter_present`) —
  a filtered list and an unfiltered one are distinguishable without recording a customer identifier;
- each failure line records the owner label, the tenant and order nil-shapes, the static owner operation,
  the correlation identifier used to join the owner's own log, `owner_code_length` instead of the owner
  error text, `retryable`, the projected `error_kind`, the selected public code, the HTTP status, and the
  boundary.

No `PortError`, message text, UUID, actor, channel, or locale value is logged.

## Preserved behavior

This work does not change:

- the `PortErrorKind` to HTTP policy mapping;
- public status, code, or message selection;
- `HttpError::new(status, code, message)` construction;
- order list and detail permissions (`ORDERS_LIST`, `ORDERS_READ`);
- owner-port selection and operations;
- list filters, pagination, locale fallback, or successful response bodies.

## Detail fan-out policy

The detail route owns two read-specific mappers, `map_payment_detail_port_error` and
`map_fulfillment_detail_port_error`. Each is defined once and called once, only from the detail route:
a read failure cannot be reported through a write envelope, and a write failure cannot be reported
through the detail ones.

They differ where the domains differ — payment `InvariantViolation` is `409`
`commerce_admin_payment_reconciliation_required` because the payment state must be reconciled before it
can be read safely, while fulfillment `InvariantViolation` is `500` `commerce_admin_fulfillment_failed`.

## Read deadline

Every read context is built by `admin_order_port_context`, which resolves the scoped resource (the order
id, or the tenant when the read is unscoped), attaches the actor, locale, and channel, and sets a two
second deadline, so a stalled owner cannot pin an admin request.

## Remaining boundary

This slice does not close raw diagnostic payloads in return/change mutations, shipping, payment,
fulfillment, product, or storefront transports. The broader ecommerce correlation-safe mapper and
non-`PortError` envelope cleanup remains open.

## Evidence

- `crates/modules/rustok-commerce/contracts/evidence/admin-order-read-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-order-read-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-order-route-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime
status is promoted.
