# Admin order read diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens `map_admin_order_port_error`, the shared `PortError` mapper used by the mounted Commerce admin order list and detail read routes.

The routes continue to call the host-selected typed order read port with the same request context, owner operation, filters, pagination, locale fallback, and resource identifiers. Only the failure diagnostic projection changes.

## Bounded diagnostic projection

The typed `PortError`, `AdminOrderErrorContext`, and `PortContext` are used for policy selection and then shadowed before `tracing::error!` by dedicated diagnostic structs.

The event no longer records the complete error, correlation ID, tenant UUID, actor UUID, order UUID, customer UUID, `PortActor`, channel, or locale. Instead it records:

- a redacted Debug representation for the error;
- the stable internal code and retryability;
- `nil` / `non_nil` shapes for required UUIDs;
- `absent` / `present_nil` / `present_non_nil` shapes for optional UUIDs;
- empty/non-empty presence shapes for correlation and actor text;
- absent/empty/non-empty presence for channel text;
- locale length and the configured deadline;
- owner operation, consumer operation, error kind, public code, HTTP status, owner, boundary, and the existing static event message.

The existing broad verifier expressions remain present, but after shadowing they reference bounded diagnostic values rather than the original request and error payloads.

## Preserved behavior

This work does not change:

- `PortErrorKind` to HTTP policy mapping;
- public status, code, or message selection;
- `HttpError::new(status, code, message)` construction;
- order list and detail permissions;
- owner-port selection and operations;
- list filters, pagination, locale fallback, or successful response bodies;
- the concrete mutation mapper;
- payment and fulfillment detail mappers.

## Remaining boundary

This slice does not close raw diagnostic payloads in the admin order mutation mapper, order-detail payment and fulfillment mappers, return/change mutations, shipping, payment, fulfillment, product, or storefront transports. The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-order-read-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-order-read-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-order-route-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime status is promoted.
