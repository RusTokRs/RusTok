# Admin product shipping-profile diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens `map_admin_product_shipping_profile_error`, the typed Commerce admin Product prevalidation mapper used before product create and update when a shipping-profile slug is supplied.

The validator continues to normalize the optional slug, return successfully when no slug is present, and call `ShippingProfileService::ensure_shipping_profile_slug_exists` with the same tenant and normalized value. Only the failure diagnostic projection changes.

## Bounded diagnostic projection

The typed `CommerceError` and `AdminProductShippingProfileErrorContext` remain available for shipping-profile-not-found identity adoption and HTTP policy selection. After those decisions, both bindings are shadowed before `tracing::error!`:

- the error becomes a diagnostic type whose Debug output is always `redacted`;
- tenant and actor identifiers become `nil` / `non_nil` shapes;
- optional product and shipping-profile identifiers become `absent` / `present_nil` / `present_non_nil` shapes;
- the static validation operation remains visible;
- error kind, public code, HTTP status, owner, boundary, and the static event message remain visible.

No slug value, owner error text, database cause, product UUID, shipping-profile UUID, tenant UUID, or actor UUID is emitted by the mapper.

## Preserved behavior

This work does not change:

- the exhaustive `CommerceError` to HTTP policy mapping;
- adoption of the UUID carried by `CommerceError::ShippingProfileNotFound`;
- `HttpError::new(status, code, message)` construction;
- absent-slug no-op and slug normalization;
- the shipping-profile owner validation call;
- product create/update permissions, inputs, catalog owner calls, or successful responses;
- the shared Admin Product mapper hardened separately in PR #3023;
- list, detail, delete, publish, or unpublish routes.

## Remaining boundary

The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open. This slice does not claim completion for return/change orchestration, order-detail enrichment, other Commerce controllers, GraphQL boundaries, owner adapters, or storefront transports.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-product-shipping-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-product-shipping-profile-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
