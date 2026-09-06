# Commerce GraphQL storefront order access owner-read cutover

Status: `source_complete_unvalidated`

## Scope

This slice continues the canonical ecommerce standalone/topology P0 after #3398 routed the five
mounted GraphQL owner-local return/change writes through `OrderPostOrderCommandPort`.

The remaining storefront ownership helper `ensure_storefront_order_access` no longer constructs a
foreign `OrderService` to read the order. It now consumes the already host-composed
`CommerceOrderReadRuntime` / `OrderReadPort` and requests the complete Order owner projection through
`ReadOrderProjectionRequest`.

## Admission and semantics

The helper still resolves the current storefront customer through the existing customer helper and
still admits the caller only when the owner projection has `customer_id == current customer_id`.
A mismatch continues to return the existing GraphQL permission-denied response.

The owner read context derives only trusted request facts:

- tenant from the resolved Commerce tenant scope supplied to the helper;
- actor from authenticated `AuthContext`;
- tenant default locale from `TenantContext`, preserving the former `OrderService::get_order` read semantics;
- channel from `RequestContext` when present;
- a bounded two-second deadline;
- a correlation identity scoped to the storefront order-access read.

The read uses the resolver-scoped host-selected `CommerceOrderReadRuntime`. Directly embedded
compatibility schemas retain the runtime helper's existing explicit in-process Order-owned fallback.

The owner projection is requested with the tenant default locale as the primary locale and no second
fallback locale. This matches the former `OrderService::get_order` behavior while moving the actual
read behind the owner port; the access check still consumes only owner identity fields.

## Error boundary

The previous helper mapped a concrete `OrderError` and logged the raw error object. The cutover maps
`PortErrorKind` directly to the existing public GraphQL Order families:

- `ORDER_REQUEST_INVALID`;
- `ORDER_RESOURCE_NOT_FOUND`;
- `ORDER_STATE_CONFLICT`;
- `ORDER_TEMPORARILY_UNAVAILABLE`;
- `ORDER_OPERATION_FAILED`.

Diagnostics retain bounded facts such as identity presence, owner error kind, owner-code length,
correlation id, public code, and retryability. Raw owner/backend messages are not logged or exposed by
this boundary.

## Deliberately unchanged

This slice does not change:

- storefront customer resolution;
- the customer/order ownership comparison;
- tenant-default-locale projection semantics;
- post-order command admission added in #3398;
- cross-domain return/change payment/refund orchestration;
- shipping-profile helper behavior in the same source file.

The broad implementation-plan item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and
Fulfillment concrete services behind host-composed owner ports` remains open. Other mounted concrete
service construction must be re-audited separately before that P0 can be checked.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted GraphQL
scenarios, workflows, CI reruns, database scenarios, restart scenarios, or remote-adapter scenarios
were executed for this slice. The accompanying verifier is source-only evidence for maintainer
execution.
