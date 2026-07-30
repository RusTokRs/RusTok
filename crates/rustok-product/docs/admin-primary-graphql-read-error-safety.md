# Product Admin primary GraphQL read error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the final public error envelope for five Product Admin reads:

- `fetch_bootstrap`;
- `fetch_products`;
- `fetch_product`;
- `fetch_product_pricing`;
- `fetch_shipping_profiles`.

The boundary lives in:

- `crates/rustok-product/admin/src/catalog_transport.rs`;
- `crates/rustok-product/admin/src/transport/graphql_error_safety.rs`.

The GraphQL adapter, query documents, variables, response DTOs, and Product owner services are unchanged.

## Confirmed gap

The Product Admin GraphQL adapter uses `rustok_graphql::GraphqlHttpError` directly.
Before this slice, the five primary read operations returned that error without a Product-owned
public policy.

Two variants carry backend-controlled detail:

- `GraphqlHttpError::Http(String)` contains HTTP status text;
- `GraphqlHttpError::Graphql(String)` contains the first GraphQL server message.

Leptos resources and UI error normalization could therefore receive those raw payloads.

## Boundary placement

A `GraphqlReadContext` is created before each selected GraphQL call.

For `fetch_products`, Product Admin keeps its existing native-first policy. The context is
created only after the native list call fails and immediately before the existing
`admin_catalog_graphql` fallback.

No new retry, fallback, transport selection, or owner call is introduced.

## Public policy

The result error type remains `GraphqlHttpError`.

| Captured condition | Public error |
| --- | --- |
| Network failure | `Network error` |
| HTTP failure | `Http error: Product admin service is temporarily unavailable` |
| Unauthorized | `Unauthorized` |
| GraphQL rejection | `GraphQL error: Product admin request could not be completed` |

`Network` and `Unauthorized` were already fixed, non-identifying variants. HTTP status text
and GraphQL server messages are replaced with static Product Admin messages.

## Internal diagnostics

The original typed `GraphqlHttpError` remains available only to the private tracing event.
Every event also records:

- owner and operation;
- a unique correlation ID;
- token presence, never the token value;
- tenant-slug presence and character length;
- tenant-ID presence and character length;
- product/resource-ID presence and character length;
- locale presence and character length;
- search and status presence and character lengths;
- currency-code presence and character length;
- whether the product-list native fallback was attempted;
- error kind, stable code, and boundary.

Raw token, tenant slug, tenant ID, product ID, locale, search, status, and currency values are
not structured fields.

## Preserved behavior

This slice does not change:

- `ProductAdminBootstrap`, `ProductList`, `ProductDetail`, `ProductPricingDetail`, or
  `ShippingProfileList`;
- GraphQL documents, variables, tenant headers, or response deserialization;
- bootstrap composition;
- list controls, category filters, sorting, or pagination;
- selected-product and pricing-preview request construction;
- shipping-profile reads;
- the product-list native-first path;
- the product-list GraphQL fallback;
- the separately merged catalog search-options String wrapper;
- FFA, FBA, browser, mounted-runtime, workflow, CI, or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-primary-read-error-safety.mjs`.

All execution flags remain `false`. Source review does not prove compilation, browser
behavior, mounted transport behavior, workflow execution, CI, or production behavior.

## Remaining work

The ecommerce correlation-safe mapper cleanup remains open for:

- Product Admin category-bound GraphQL fallback reads;
- Product Admin GraphQL writes and status mutations;
- other ecommerce adapters and non-`PortError` public envelopes;
- runtime and mounted transport evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-product-admin-primary-read-error-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
