# Admin order mutation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice covers the four mounted Commerce admin order mutations — mark paid, ship, deliver, and
cancel — as they are served today by `controllers/admin/orders_owner_ports.rs` through
`OrderAdminCommandPort`.

The controller used to be duplicated in `controllers/admin/orders.rs`, a file that was never part of
the module tree: `controllers/admin/mod.rs` mounts `orders_owner_ports.rs` under the `orders` module
name, so the second copy could drift, keep obsolete `OrderService` calls, and still be cited by
verifiers as if it were the production path. That file is deleted; the guard now pins the mounted
controller and fails if the duplicate returns.

## Bounded diagnostic projection

Every mutation calls the host-selected `OrderAdminCommandPort` with a typed `Owner*Request`, and every
failure funnels into the single `map_order_port_error` mapper. The mapper writes one `tracing::error!`
line that contains only bounded facts:

- tenant and actor identifiers as `nil` / `non_nil` shape (`tenant_id_non_nil`, `actor_id_non_nil`);
- the order identifier as presence plus shape (`order_id_present`, `order_id_non_nil`);
- the static consumer operation, the correlation identifier, the boundary, and the owner label;
- the owner error `code` as a length (`owner_code_length`), never as text;
- `retryable`, the projected `error_kind`, the selected public code, and the HTTP status.

No `OrderError`, `PortError`, message text, payload, or raw UUID reaches the log or the response.

## Preserved behavior

This work does not change:

- the `PortErrorKind` to HTTP policy mapping, including `Unavailable | Timeout` collapsing to `503`
  `commerce_admin_order_storage_unavailable` and `InvariantViolation` to `500`
  `commerce_admin_order_failed`;
- public status, code, or message selection;
- `HttpError::new(status, code, message)` construction;
- the four mutation routes or their `ORDERS_UPDATE` permission;
- the typed owner request shapes forwarded to the command port;
- the shared mapper also used by the admin order list and detail routes;
- the order-detail payment and fulfillment mappers.

## Write identity

All four mutations require a caller-owned `Idempotency-Key` header, validated to 1..=191 bytes, and
forward it into the port context via `admin_order_command_port_context`. The key is never generated
inside the handler, so a retry stays a retry instead of becoming a second write.

## Remaining boundary

This slice does not close raw diagnostic payloads in order-detail payment and fulfillment mappers,
return/change mutations, shipping, payment, fulfillment, product, or storefront transports. The
broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.

## Evidence

- `crates/modules/rustok-commerce/contracts/evidence/admin-order-mutation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-order-mutation-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-admin-order-route-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime
status is promoted.
