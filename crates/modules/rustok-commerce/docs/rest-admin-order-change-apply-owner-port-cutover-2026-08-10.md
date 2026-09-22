# Commerce REST admin order-change apply owner-port cutover

Status: `source_complete_unvalidated`

Date: 2026-08-10

## Scope

This bounded slice moves mounted `POST /admin/order-changes/{id}/apply` onto host-selected
Order owner ports for the Order-owned parts of the workflow.

The REST controller now composes `OrderChangeOrchestrationService::from_order_ports` with:

- `CommerceHttpRuntime::order_read_port()`;
- `CommerceHttpRuntime::order_post_order_command_port()`.

The orchestration then reads the order-change projection through `OrderReadPort` and uses
`OrderPostOrderCommandPort::apply_change` for the ordinary/default apply transition.
Commerce no longer constructs a concrete Order service on the mounted REST apply path for
those operations.

## Owner command capability

Order now publishes typed `ApplyOrderChangeRequest` through
`OrderPostOrderCommandPort::apply_change`.

The in-process owner adapter delegates to the existing `OrderService::apply_order_change`, so
Order remains responsible for lifecycle validation, persistence, event publication and the
existing `OrderChangeResponse` projection.

The new trait method has a default fail-closed implementation. Existing external adapters remain
source-compatible, but an adapter that has not explicitly implemented order-change apply returns
stable unavailable `PortError` after normal write-port policy admission.

## REST owner contexts

The mounted REST route keeps `orders:update` permission admission and now accepts the standard
`RequestContext`.

The read context carries:

- admitted tenant id;
- authenticated user as `PortActor::user`;
- request locale and optional channel;
- order-change-bound correlation identity;
- a two-second read deadline.

The command context carries the same request identity plus a two-second write deadline and a fresh
UUID idempotency identity. This route does not expose a caller idempotency key, so the UUID is
**write-admission metadata only** and this slice does not claim durable replay or exactly-once
semantics.

## Error boundary

Owner read and command failures remain `PortError` through the Commerce orchestration boundary.
The REST controller maps them to the existing admin Order public families:

- validation -> `400 commerce_admin_order_invalid`;
- not found -> `404 commerce_admin_not_found`;
- state conflict -> `409 commerce_admin_order_state_conflict`;
- unavailable/timeout -> `503 commerce_admin_order_storage_unavailable`;
- invariant failure -> `500 commerce_admin_order_failed`;
- defensive forbidden -> `401 commerce_permission_denied`.

The owner-port mapper records only bounded error kind, owner-code length, retryability,
correlation and request-identity shape. It does not log or return the raw owner/backend message.

Existing exchange/claim Payment orchestration keeps its previous typed
`PostOrderOrchestrationError` mapping and public envelopes.

## Explicitly outside this slice

Two related paths remain separate work:

1. mounted GraphQL `applyOrderChange` still obtains the compatibility
   `OrderChangeOrchestrationService::new` path through `order_change_orchestration_from_context`;
   its host-selected Order read/command composition is intentionally deferred to a dedicated
   GraphQL cutover;
2. exchange and claim branches still delegate to `PostOrderOrchestrationService`, whose broader
   cross-domain owner-port decomposition is not changed here.

Compiled legacy create/list/show/cancel functions in `controllers/admin/changes.rs` also remain
as compatibility source, but `controllers/admin/mod.rs` mounts `post_order_commands` and
`post_order_reads` for those routes instead.

The canonical broad ecommerce topology P0 therefore remains open.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, REST/GraphQL
scenarios, workflows, CI reruns, database scenarios, provider calls, restart scenarios or
remote-adapter scenarios were executed. Source guards added or updated by this slice are
source-reviewed only and were not run.
