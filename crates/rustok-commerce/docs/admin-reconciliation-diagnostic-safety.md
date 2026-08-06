# Admin reconciliation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the complete Commerce Admin Fulfillment Reconciliation HTTP boundary:

- list operations requiring reconciliation;
- quarantine stale executing operations;
- resolve an unknown provider operation as failed;
- resolve an unknown provider operation as succeeded;
- retry local fulfillment persistence;
- retry provider label creation.

The six mounted routes, their permissions, owner calls, provider composition, limits, stale-time clamp, and successful response DTOs remain unchanged.

## Bounded diagnostic projection

Each route now supplies typed tenant, actor, optional provider-operation identity, and a static route-operation label. Typed `FulfillmentError`, `FulfillmentOrchestrationError`, and `serde_json::Error` values remain available until the mapper selects the existing HTTP policy.

Before each `tracing::error!` event:

- the typed error is shadowed by a diagnostic type whose `Debug` output is always `redacted`;
- tenant and actor UUIDs become `nil` / `non_nil`;
- optional provider-operation identity becomes `absent` / `present_nil` / `present_non_nil`;
- owner, route operation, error kind, public code, HTTP status, boundary, and static event message remain observable.

No validation detail, database cause, provider payload, transition detail, serialization detail, UUID, or actor identifier is emitted by these mappers.

## Preserved HTTP policy

This work does not change:

- fulfillment validation, not-found, state-conflict, and storage-unavailable status/code/message policy;
- orchestration order-not-found, validation, storage-unavailable, and reconciliation-required policy;
- nested fulfillment-error delegation;
- fail-closed provider-result encoding status, code, and static message;
- `HttpError::new(status, code, message)` construction.

## Preserved route behavior

This work does not change:

- `FULFILLMENTS_MANAGE` authorization;
- `FulfillmentProviderOperationRecovery` list, quarantine, and resolve calls;
- `FulfillmentReconciliationService` local-persistence retry;
- `FulfillmentCreateLabelRecoveryService` provider retry;
- fulfillment provider-registry composition;
- the default limit of 100;
- the stale interval clamp from 60 seconds through seven days;
- successful provider-operation and fulfillment response envelopes.

## Remaining boundary

The broad ecommerce correlation-safe mapper and non-`PortError` public-envelope cleanup remains open. Storefront shared context/customer/channel, storefront cart shipping, inventory, tax, promotion, remaining owner adapters, GraphQL/native transports, and runtime verification are not completed by this slice.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-reconciliation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-fulfillment-reconciliation-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
