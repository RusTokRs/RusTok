# Admin order mutation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens `map_admin_order_error`, the shared typed `OrderError` mapper used by the four mounted Commerce admin order mutations: mark paid, ship, deliver, and cancel.

The routes continue to call the concrete order owner with the same tenant, actor, order, input, permission, and operation values. Only the failure diagnostic projection changes.

## Bounded diagnostic projection

The typed `OrderError` and `AdminOrderErrorContext` remain available for order-not-found identity adoption and HTTP policy selection. After those decisions, both bindings are shadowed before `tracing::error!`:

- the error becomes a diagnostic type whose Debug output is always `redacted`;
- tenant and actor identifiers become `nil` / `non_nil` shapes;
- optional order and customer identifiers become `absent` / `present_nil` / `present_non_nil` shapes;
- the static consumer operation remains visible;
- error kind, public code, HTTP status, owner, boundary, and static event message remain visible.

The existing broad verifier expressions remain present, but they reference bounded shadow values at the log site rather than the original typed error and UUID payloads.

## Preserved behavior

This work does not change:

- `OrderError` to HTTP policy mapping;
- adoption of the UUID carried by `OrderError::OrderNotFound`;
- public status, code, or message selection;
- `HttpError::new(status, code, message)` construction;
- the four mutation routes or their `ORDERS_UPDATE` permission;
- mutation owner calls or forwarded inputs;
- the admin order read mapper;
- order-detail payment and fulfillment mappers.

## Remaining boundary

This slice does not close raw diagnostic payloads in order-detail payment and fulfillment mappers, return/change mutations, shipping, payment, fulfillment, product, or storefront transports. The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-order-mutation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-order-mutation-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-order-route-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime status is promoted.
