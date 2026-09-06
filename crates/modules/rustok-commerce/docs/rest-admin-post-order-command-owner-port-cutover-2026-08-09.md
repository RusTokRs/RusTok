# Commerce REST admin post-order command owner-port cutover

Status: `source_complete_unvalidated`

Date: 2026-08-09

## Scope

This bounded slice moves four mounted admin REST Order writes behind the existing
Order-owned `OrderPostOrderCommandPort`:

- `POST /admin/orders/{id}/changes` -> `create_change`;
- `POST /admin/order-changes/{id}/cancel` -> `cancel_change`;
- `POST /admin/orders/{id}/returns` -> `create_return`;
- `POST /admin/returns/{id}/cancel` -> `cancel_return`.

The owner capability already delegates these operations to the canonical Order
service inside `rustok-order`; Commerce no longer constructs that concrete owner
service on these mounted routes.

## Host composition

`apps/server/src/services/commerce_provider_runtime.rs` already composes
`OrderPostOrderCommandRuntime`, preserving a runtime supplied by either
`HostRuntimeContext` or `ServerRuntimeContext` before installing the deterministic
in-process baseline.

`CommerceHttpRuntime` now requires that host-selected runtime and exposes only its
`Arc<dyn OrderPostOrderCommandPort>` to the mounted handlers. The REST path therefore
uses the same host-selected owner capability already required by mounted Commerce
GraphQL post-order commands.

## Request context and write admission

Each mounted REST call preserves `ORDERS_UPDATE` admission and constructs a bounded
`PortContext` with:

- the admitted tenant;
- the authenticated user actor;
- the request locale;
- the request channel when present;
- a resource/operation-bound correlation id;
- a two-second deadline;
- a non-empty generated idempotency identity required by the owner write policy.

These legacy REST endpoints do not expose a caller idempotency key. The generated
UUID is therefore write-admission metadata only. This slice does **not** claim
durable replay or exactly-once semantics for repeated HTTP requests.

## Preserved public behavior

The success status and DTO shapes remain unchanged:

- create change / create return -> `201 Created`;
- cancel change / cancel return -> `200 OK`.

The owner `PortError` mapper preserves the existing REST error families for the
Order variants previously exposed by the direct service path:

- validation -> `400 commerce_admin_order_invalid`;
- missing Order/change/return -> `404 commerce_admin_not_found`;
- lifecycle conflict -> `409 commerce_admin_order_state_conflict`;
- unavailable/timeout -> `503 commerce_admin_order_storage_unavailable`;
- invariant failure -> `500 commerce_admin_order_failed`.

A host-selected owner may additionally return `Forbidden`; that fails closed through
the existing `401 commerce_permission_denied` family after Commerce permission
admission.

Diagnostics include only bounded owner kind/code-length/retryability/correlation and
non-nil identity facts. `PortError.message` and raw backend/owner causes are not logged
or exposed by the new mounted boundary.

## Deliberately unchanged

Payment-coupled post-order orchestration is outside this owner-command slice and stays
on its existing Commerce services:

- return decision creation;
- order-change apply;
- return completion.

The old `changes.rs` and `returns.rs` create/cancel functions remain compiled
compatibility source, but `admin::axum_router()` no longer mounts those four legacy
handlers. Existing post-order GET routes continue to use `post_order_reads`.

## Topology status

The canonical broad item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment,
and Fulfillment concrete services behind host-composed owner ports` remains open.
Other mounted ecommerce orchestration/consumer paths still require audit and cutover.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, REST
scenarios, workflows, CI reruns, database scenarios, provider calls, restart scenarios,
or remote-adapter scenarios were executed. The dedicated source verifier added with
this slice was source-reviewed but not run.
