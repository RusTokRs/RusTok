# Commerce admin Fulfillment create owner-port cutover — 2026-08-09

## Status

Source-complete for the mounted `POST /admin/fulfillments` route. Execution evidence remains pending and unvalidated.

This slice intentionally keeps manual-fulfillment policy orchestration in Commerce while removing direct Order ORM, concrete Fulfillment service, and direct provider execution from the mounted create path.

## Mounted route

The mounted admin route remains:

- `POST /admin/fulfillments`
- permission: `FULFILLMENTS_CREATE`
- request: existing `CreateFulfillmentInput`
- success: HTTP 201 with existing `FulfillmentResponse`

The mounted `fulfillments_owner_commands.rs` adapter now defines a local `create_fulfillment` handler. That local item shadows the compatibility re-export from `fulfillments_legacy`, preserving router/OpenAPI names without changing the large admin router.

## Cross-owner policy remains in Commerce

`AdminManualFulfillmentOrchestrationService` owns the cross-owner policy composition. It uses only typed owner capabilities:

- `rustok_order::OrderReadPort`
- `rustok_fulfillment::FulfillmentReadPort`
- `rustok_fulfillment::ShippingOptionReadPort`
- `rustok_fulfillment::FulfillmentAdminCreateCommandPort`

The orchestration retains the pre-cutover rules:

- the order must exist in the tenant;
- manual fulfillment requires typed non-empty `items[]`;
- an explicitly supplied customer must match the order customer;
- requested line items must belong to the order;
- already fulfilled non-cancelled quantities are subtracted before admission;
- legacy fulfillments without typed items fail closed;
- all requested items must belong to one seller-aware delivery group;
- shipping-profile slugs use the existing Commerce normalization/fallback policy;
- seller identity still falls back to legacy line-item metadata when the typed field is absent;
- a selected shipping option must match the order currency and required shipping profile;
- prepared fulfillment item/delivery-group metadata retains the existing `post_order.manual = true` facts.

The service does not import SeaORM Order entities and does not construct `FulfillmentService`.

## Fulfillment-owned create execution

`rustok-fulfillment` now publishes:

- `FulfillmentAdminCreateCommandPort`
- `FulfillmentAdminCreateCommandRuntime`
- `InProcessFulfillmentAdminCreateCommandPort`
- `CreateAdminFulfillmentRequest`

The in-process owner adapter owns:

- `FulfillmentService` construction;
- selected shipping-option/provider consistency validation;
- fulfillment persistence;
- `FulfillmentProviderOperationJournal` construction;
- create-label provider execution through the host-selected `FulfillmentProviderRegistry`.

The owner does not import Commerce or Order.

## Create-label replay identity

The provider operation record remains the durable external create-label execution anchor. Its `fulfillment_id`
points to the same local Fulfillment created in the owner transaction, while
`(tenant_id, provider_id, idempotency_key)` is the durable provider-operation request namespace. The caller-owned
`Idempotency-Key` is forwarded unchanged to provider execution.

The local Fulfillment row and its `create_label` provider operation are inserted in one owner transaction before
external provider execution begins. A committed pending operation therefore always has its local anchor; a
transaction rollback removes both. Existing committed/provider-succeeded journal rows are adopted rather than
re-executed, and reconciliation rows with a valid persisted provider result remain adoptable.

Provider-result serialization failure is hardened in the owner path: it explicitly records
reconciliation-required state instead of leaving the journal in an unresolved executing state. This does not
change the public 409 reconciliation envelope.

## Transport write identity

The mounted route supplies the caller-owned `Idempotency-Key` through `PortContext`. The owner uses it as the
provider-operation idempotency identity; no payload-derived or `fulfillment_id`-derived synthetic provider key
is generated.

## Runtime composition

`CommerceHttpRuntime` prefers a host-injected `FulfillmentAdminCreateCommandRuntime`. If absent, the built-in in-process Fulfillment owner adapter is composed with the same host-selected `FulfillmentProviderRegistry` already used by the other admin Fulfillment command runtime.

## Public error compatibility

The mounted route keeps the existing public families:

- validation -> 400 `commerce_admin_fulfillment_invalid`;
- missing Order/Fulfillment/ShippingOption resource -> 404 `commerce_admin_not_found`;
- create-label/provider outcome requiring reconciliation after fulfillment persistence -> 409 `commerce_admin_fulfillment_reconciliation_required`;
- storage/unavailable -> 503 `commerce_admin_fulfillment_storage_unavailable`;
- forbidden -> existing permission-denied family;
- invariant failure -> 500 `commerce_admin_fulfillment_failed`.

## Still open

The canonical broad Commerce topology P0 remains open. This slice does not remove concrete owner construction from remaining post-order/change/return, GraphQL/provider-operation, checkout, reconciliation, or other Commerce surfaces.

The legacy Commerce fulfillment orchestration files are retained because other workflows still reference them and because they document compatibility behavior. They are not evidence that the mounted admin create route still uses direct concrete construction.

## Validation status

Source/GitHub inspection only. Tests, Cargo commands, formatting, verifier execution, workflows, CI, runtime HTTP calls, restart/lost-response evidence, and external-provider execution were intentionally not run.
