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

The durable provider operation identity remains exactly:

```text
fulfillment:{fulfillment_id}:create_label
```

The provider operation kind remains `create_label`, and provider request metadata retains:

```text
commerce_orchestration.operation = "create_label"
```

Existing committed/provider-succeeded journal rows are adopted rather than re-executed. Reconciliation rows with a valid persisted provider result remain adoptable; unresolved/invalid provider outcome state returns the existing reconciliation-required public family.

Provider-result serialization failure is hardened in the owner path: it explicitly records reconciliation-required state instead of leaving the journal in an unresolved executing state. This does not change the public 409 reconciliation envelope.

## Transport write identity is not a durable create receipt

The mounted route supplies a stable input-sensitive `PortContext` idempotency key based on the public create request and the existing dual FNV-1a offset pattern. This satisfies generic write-port admission and gives host-injected adapters stable transport identity.

It is **not** a newly claimed durable manual-fulfillment creation receipt. The in-process durable replay boundary in this slice remains the Fulfillment-owned create-label provider journal after the fulfillment row has been created.

Retained lost-response/restart evidence for the whole create command is still required before claiming end-to-end durable command idempotency.

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
