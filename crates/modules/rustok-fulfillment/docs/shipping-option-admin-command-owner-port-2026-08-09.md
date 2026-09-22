# Fulfillment shipping-option admin command owner port

Status: `source_complete_unvalidated`

## Scope

This slice publishes the missing Fulfillment-owned command boundary required to remove mounted
Commerce REST construction of `FulfillmentService` for shipping-option writes.

`ShippingOptionAdminCommandPort` now owns the four owner-local operations used by Commerce admin
shipping routes:

- create shipping option;
- update shipping option;
- deactivate shipping option;
- reactivate shipping option.

The public `ShippingOptionAdminCommandRuntime` accepts an injected owner implementation and exposes
an explicit in-process adapter for hosts that select the built-in Fulfillment implementation.

## Admission and ownership

Every command requires the canonical write `PortCallPolicy`. The tenant UUID is parsed only from the
admitted `PortContext`; the in-process adapter does not accept a separate transport tenant argument.

This capability does **not** claim durable idempotent replay for shipping-option create/update/state
writes. A caller or host may carry an idempotency identity in `PortContext`, but this source slice does
not consume it as an owner receipt. Durable replay semantics must be added explicitly before any
exactly-once claim is made.

The in-process implementation keeps `FulfillmentService` construction inside `rustok-fulfillment`.
Shipping-option validation, persistence, activation state, translations, provider id, metadata, and
response projection therefore remain owner-local.

The request wrappers preserve the existing owner DTOs:

- `CreateShippingOptionInput`;
- `UpdateShippingOptionInput`;
- `ShippingOptionResponse`.

This slice does not add a new transport shape.

## Bounded errors

Owner `FulfillmentError` values are mapped to stable `PortError` families without exposing raw
validation/database text:

- validation -> `fulfillment.validation`;
- missing shipping option -> `fulfillment.shipping_option_not_found`;
- state conflict -> `fulfillment.invalid_transition`;
- storage failure -> `fulfillment.database_unavailable`.

Diagnostics retain bounded context/error facts and the stable public owner code. Raw backend errors
are not logged by this boundary.

## Deliberately still open

Mounted Commerce admin REST `create_shipping_option`, `update_shipping_option`,
`deactivate_shipping_option`, and `reactivate_shipping_option` still construct `FulfillmentService`
directly in this capability-only slice. The next consumer cutover must host-compose
`ShippingOptionAdminCommandRuntime` and route those four handlers through the new owner port while
preserving shipping-profile validation, permissions, request context, responses, and public HTTP
errors.

The canonical ecommerce topology item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and
Fulfillment concrete services behind host-composed owner ports` remains open.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted REST
scenarios, workflows, CI reruns, database scenarios, restart scenarios, or remote-adapter scenarios
were executed for this slice. The accompanying verifier is source-only evidence for maintainer
execution.
