# Commerce GraphQL Fulfillment command owner-port cutover

Status: `source_complete_unvalidated`

## Scope

This slice continues the canonical ecommerce topology P0 by moving the mounted Commerce GraphQL Fulfillment provider-operation mutations behind host-composed owner ports while preserving the existing public GraphQL fields, permissions, DTO shapes, and public error families.

Mounted fields covered by this slice:

- `createFulfillment`
- `shipFulfillment`
- `deliverFulfillment`
- `reopenFulfillment`
- `reshipFulfillment`
- `cancelFulfillment`

## Owner boundaries

Mounted lifecycle mutations now resolve `CommerceFulfillmentCommandRuntime` from `CommerceGraphqlRuntimeData` and call `FulfillmentAdminCommandPort`. The runtime prefers host-supplied `FulfillmentAdminCommandRuntime` and falls back to the Fulfillment-owned in-process adapter with the deployment-selected provider registry.

`createFulfillment` keeps cross-owner order/shipping policy in Commerce, but the policy service is composed exclusively from the existing `OrderReadPort`, `FulfillmentReadPort`, `ShippingOptionReadPort`, and `FulfillmentAdminCreateCommandPort` capabilities. Mounted schemas therefore no longer construct a concrete Fulfillment service for manual creation. Directly embedded compatibility schemas that omit manifest runtime data retain the pre-existing concrete orchestration fallback only for that compatibility case.

Provider journal ownership remains in `rustok-fulfillment`. This slice does not reimplement provider execution or persistence in GraphQL. The existing owner code continues to own create-label journaling/reconciliation and lifecycle provider-operation journaling/reconciliation.

## Request context and public errors

GraphQL builds typed `PortContext` values from validated `AuthContext` and `RequestContext` data. Lifecycle commands receive deterministic transport idempotency identities and two-second deadlines. Manual creation uses separate read/write contexts and an input-sensitive deterministic write idempotency identity, matching the established admin HTTP cross-owner create pattern.

Owner `PortError` values are mapped back to the existing GraphQL Fulfillment public families:

- `ORDER_RESOURCE_NOT_FOUND`
- `FULFILLMENT_REQUEST_INVALID`
- `FULFILLMENT_RESOURCE_NOT_FOUND`
- `FULFILLMENT_STATE_CONFLICT`
- `FULFILLMENT_TEMPORARILY_UNAVAILABLE`
- `FULFILLMENT_RECONCILIATION_REQUIRED`

Diagnostics record only bounded owner/error metadata, correlation identity, non-nil resource facts, and public classification; owner messages and opaque payloads are not exposed.

## Canonical plan state

The broad canonical topology item remains open. Other mounted Commerce concrete-owner paths, post-order/change/return orchestration, checkout/reconciliation work, and runtime verification gates are outside this slice.

## Validation

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, CI, runtime calls, provider execution, lost-response scenarios, restart scenarios, or remote-adapter scenarios were executed for this slice. The maintainer explicitly requested to run tests independently.
