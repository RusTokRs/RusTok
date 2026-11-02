# Commerce GraphQL Fulfillment command owner-port cutover

Status: `source_complete_unvalidated`

## Scope

This slice continues the canonical ecommerce topology P0 by moving the mounted Commerce GraphQL Fulfillment provider-operation mutations behind host-composed owner ports. Field names, permissions, input DTOs and public error families remain; the 2026-11-01 continuation adds a required caller-owned `idempotencyKey` argument to the five lifecycle fields listed below.

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

GraphQL builds typed `PortContext` values from validated `AuthContext` and `RequestContext` data, checking Auth/Tenant/target tenant agreement. It preserves canonical `auth.port_actor()` (human versus OAuth service), permissions/scopes, locale/channel and actual request correlation; only directly embedded schemas without RequestContext retain the resource-scoped correlation fallback. AuthContext has no roles field.

Each of `shipFulfillment`, `deliverFulfillment`, `reopenFulfillment`, `reshipFulfillment` and `cancelFulfillment` requires a caller-owned `idempotencyKey` of 1..=191 UTF-8 bytes and uses a two-second call budget. A distinct later intent must use a distinct key; uncertain retries retain key and payload. The owner journal uses that key literally, rather than regenerating the old per-resource or FNV lifecycle identity. Local recovery alone loads deployed historical keys from its scoped saved journal. Manual creation keeps separate read/write contexts and its existing input-sensitive deterministic GraphQL create identity; it is not the lifecycle key policy.

Native lifecycle composition now receives the selected Order reader for saved SKU preparation. Host-selected F commands still win, and unavailable selected dependencies are never converted to Noop. See the [shipment handoff](implementation-plan.md#shipment-stock-provenance-source-evidence) for the prepared stock protocol, caller transaction and honest provider-outcome boundary.

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

The original 2026-08-09 slice carried no execution evidence. The 2026-11-01 continuation runs only read-only source guards, source syntax parsing and narrow WASM formatting. Source guards, syntax parsing and formatting are not compilation or runtime evidence.

Tests remain unrun and with the maintainer. No Rust typecheck, SQL/carrier execution, mounted GraphQL scenario, CI/workflow, lost-response/restart or remote-adapter validation is claimed. The broad topology and Product step-8 gates remain open.
