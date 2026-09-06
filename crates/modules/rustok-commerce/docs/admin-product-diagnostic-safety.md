# Admin product diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens `map_admin_product_error`, the shared typed `CommerceError` mapper used by the mounted Commerce admin product reads and mutations.

The mapper remains shared by list count/page/translation/tag reads, product detail, create, update, delete, publish, and unpublish. Routes continue to use the same permissions, tenant and actor inputs, locale fallback, filters, pagination, metrics, shipping-profile validation, catalog owner calls, and success responses. Only the failure diagnostic projection changes.

## Bounded diagnostic projection

The typed `CommerceError` and `AdminProductErrorContext` remain available for product-not-found identity adoption and HTTP policy selection. After those decisions, both bindings are shadowed before `tracing::error!`:

- the error becomes a diagnostic type whose Debug output is always `redacted`;
- tenant and actor identifiers become `nil` / `non_nil` shapes;
- the optional product identifier becomes `absent` / `present_nil` / `present_non_nil`;
- the static consumer operation remains visible;
- error kind, public code, HTTP status, owner, boundary, and static event message remain visible.

No database cause, core error, validation text, duplicate handle or SKU value, or raw UUID is retained at this log site.

## Preserved behavior

This work does not change:

- `CommerceError` to HTTP policy mapping;
- adoption of the UUID carried by `CommerceError::ProductNotFound`;
- public status, code, or message selection;
- `HttpError::new(status, code, message)` construction;
- the ten shared mapper callsites across the shared and admin product controllers;
- product route permissions and owner calls;
- list filters, locale fallback, pagination, metrics, tag loading, or response DTOs;
- the separate shipping-profile validation mapper.

## Recheck note

The older aggregate `verify-commerce-admin-product-route-error-context.mjs` guard still describes an earlier, wider product enum/context shape than current `main`. This slice does not claim that stale aggregate guard is executable; the focused guard below locks the current source contract without rewriting unrelated historical expectations.

## Remaining boundary

This slice does not close raw diagnostic payloads in the separate shipping-profile validation mapper, return/change mutations, pricing GraphQL adapters, payment, fulfillment, or storefront transports. The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-product-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-product-diagnostic-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run. No compile or runtime status is promoted.
