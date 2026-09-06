# Admin shipping diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the three failure mappers used by the Commerce Admin Shipping HTTP boundary:

- shipping-profile owner operations and shipping-profile slug validation;
- direct fulfillment-owner shipping-option mutations;
- host-composed shipping-option list and detail reads.

All twelve mounted shipping-profile and shipping-option routes remain unchanged. Only the diagnostic projection emitted after typed HTTP policy selection changes.

## Bounded diagnostic projection

Typed `CommerceError`, `FulfillmentError`, `PortError`, `PortContext`, and `AdminShippingOptionErrorContext` remain available while HTTP policy is selected. Before each `tracing::error!` event:

- the typed error is shadowed by a diagnostic type whose `Debug` output is always `redacted`;
- tenant identity becomes `nil` / `non_nil`;
- optional shipping-option identity becomes `absent` / `present_nil` / `present_non_nil`;
- correlation, actor, and channel values become closed presence-shape labels;
- locale becomes its length rather than its content;
- deadline, stable internal code, retryability, owner operation, route operation, error kind, public code, HTTP status, owner, boundary, and static event messages remain observable where they existed before.

No validation text, database cause, provider payload, transition detail, UUID, actor identifier, correlation identifier, channel value, or locale value is emitted by these mappers.

## Preserved behavior

This work does not change:

- shipping-profile `CommerceError` status/code/message policy;
- shipping-option `FulfillmentError` mutation policy;
- shipping-option `PortErrorKind` read policy;
- `HttpError::new(status, code, message)` construction;
- `FULFILLMENTS_READ`, `FULFILLMENTS_CREATE`, and `FULFILLMENTS_UPDATE` permissions;
- profile slug validation before option create/update;
- shipping-profile list/create/show/update/deactivate/reactivate owner calls;
- host-composed shipping-option list/detail reads;
- direct shipping-option create/update/deactivate/reactivate owner calls;
- active, currency, provider, and search filters;
- pagination, locale forwarding, tenant default-locale fallback, and successful response DTOs.

## Remaining boundary

The broad ecommerce correlation-safe mapper and non-`PortError` public-envelope cleanup remains open. This slice does not claim completion for inventory, customer, tax, promotion, checkout-operation, storefront, native, GraphQL, owner-adapter, or runtime verification work.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-shipping-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-shipping-option-error-context.mjs`
- `scripts/verify/verify-commerce-admin-shipping-http-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
