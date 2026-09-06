# Commerce GraphQL shipping-option command owner-port cutover

Status: `source_complete_unvalidated`

## Scope

The four mounted Commerce GraphQL shipping-option writes now execute through the Fulfillment-owned
`ShippingOptionAdminCommandPort` / `ShippingOptionAdminCommandRuntime` published in the preceding
owner-capability slice.

The cutover covers the mounted `CommerceCheckoutMutation` fields backed by
`graphql/mutations/checkout.rs`:

- `createShippingOption`;
- `updateShippingOption`;
- `deactivateShippingOption`;
- `reactivateShippingOption`.

Those resolver paths no longer construct `rustok_fulfillment::FulfillmentService`.

## Host and schema composition

The application server now composes `ShippingOptionAdminCommandRuntime` into the shared
`HostRuntimeContext`, preserving a runtime supplied by the host or `ServerRuntimeContext` and using
the Fulfillment-owned in-process adapter only when no external runtime was selected.

`CommerceGraphqlRuntimeData` requires that shared runtime for mounted schema construction and exposes
it to checkout resolvers. `shipping_option_admin_command_runtime_from_context` retains an explicit
in-process fallback only for directly embedded compatibility schemas that do not install mounted
Commerce GraphQL runtime data.

The existing Commerce HTTP runtime also consumes `ShippingOptionAdminCommandRuntime`, so the normal
server composition now selects one owner runtime for both mounted REST and GraphQL shipping-option
write transports.

## Preserved admission and request semantics

Existing Commerce permission and tenant admission remains in front of every owner call:

- create requires `FULFILLMENTS_CREATE`;
- update/deactivate/reactivate require `FULFILLMENTS_UPDATE`;
- `current_tenant_scope` remains authoritative for the admitted tenant;
- create/update still validate Commerce-owned shipping-profile slugs before crossing the owner
  boundary.

Each command receives a trusted `PortContext` derived from `AuthContext` and `RequestContext` with:

- authenticated user actor;
- admitted tenant id;
- request locale, falling back to `und` only when unavailable/blank;
- request channel when present;
- operation/resource correlation identity;
- a two-second deadline;
- a fresh non-empty idempotency identity required by the canonical write policy.

The fresh GraphQL idempotency identity is admission metadata only. This slice does **not** claim
durable shipping-option receipt/replay or exactly-once semantics; the owner capability still does
not persist command receipts for these operations.

## Error boundary

The mounted safe checkout facade now consumes bounded `PortError` rather than concrete
`FulfillmentError` for these four owner calls. Public GraphQL compatibility is preserved:

- validation -> `SHIPPING_OPTION_REQUEST_INVALID`;
- owner shipping-option not found -> `SHIPPING_OPTION_NOT_FOUND`;
- conflict -> `SHIPPING_OPTION_STATE_CONFLICT`;
- unavailable/timeout -> `SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE`;
- unexpected not-found/forbidden/invariant failures -> `SHIPPING_OPTION_OPERATION_FAILED`.

Diagnostics contain only bounded context, owner kind/code length, retryability, operation, and public
classification. Raw owner/backend messages are not logged or exposed by this boundary.

## Deliberately still open

Shipping-profile CRUD remains Commerce-owned and intentionally continues to use
`ShippingProfileService`.

`createStorefrontPaymentCollection` in the same resolver source still constructs `PaymentService`;
that mounted Payment topology gap is a separate owner-port slice.

The canonical ecommerce topology item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and
Fulfillment concrete services behind host-composed owner ports` remains open.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted GraphQL
scenarios, workflows, CI reruns, database scenarios, restart scenarios, provider calls, or
remote-adapter scenarios were executed for this slice. The accompanying verifier and updated
checkout error-safety verifier are source-only evidence for maintainer execution.
