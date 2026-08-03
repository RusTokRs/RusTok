# Product Admin primary GraphQL read error safety

Status: **source-ready / unvalidated**

## Scope

This contract covers five Product Admin primary reads:

- `fetch_bootstrap`;
- `fetch_products`;
- `fetch_product`;
- `fetch_product_pricing`;
- `fetch_shipping_profiles`.

The public boundary remains in `catalog_transport.rs`; typed classification and private diagnostics remain in `transport/graphql_error_safety.rs`.

## Public policy

The result type remains `GraphqlHttpError`.

| Captured condition | Public error |
| --- | --- |
| Network failure | `Network error` |
| HTTP failure | `Http error: Product admin service is temporarily unavailable` |
| Unauthorized | `Unauthorized` |
| GraphQL rejection | `GraphQL error: Product admin request could not be completed` |

HTTP status text and GraphQL server messages do not cross the Product Admin public boundary.

## Diagnostic recheck

The public mapper was already static and correlation-aware, but both tracing severity branches still recorded `raw_error = ?error`. Because `Http(String)` and `Graphql(String)` carry backend-controlled text, the complete typed error remained a structured diagnostic payload.

The read mapper now records only payload presence and character length for `Http` and `Graphql`. `Network` and `Unauthorized` retain no invented payload. The complete typed error is not logged.

The event still retains:

- owner, operation, boundary, stable code, and closed error kind;
- a unique correlation ID;
- token presence and bounded tenant/resource/locale/search/status/currency shape;
- whether product-list native fallback was attempted;
- technical-error versus ordinary-rejection severity.

Raw request values, HTTP text, GraphQL server messages, and Debug representations of the complete `GraphqlHttpError` are not emitted by the read boundary.

## Preserved behavior

This change does not alter:

- public messages, codes, retry classification, or result types;
- GraphQL documents, variables, tenant headers, or response deserialization;
- bootstrap composition;
- product list controls, category filters, sorting, or pagination;
- product-list native-first execution or GraphQL fallback;
- selected-product, pricing-preview, or shipping-profile request construction;
- UI resource composition;
- retry or fallback count;
- Product FFA/FBA, browser, mounted-runtime, workflow, CI, or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs`;
- compatibility command `scripts/verify/verify-product-admin-primary-read-error-safety.mjs`.

All execution flags remain `false`. Source review does not prove compilation, verifier execution, browser behavior, mounted transport behavior, workflow execution, CI, or production behavior.

## Remaining work

The ecommerce correlation-safe mapper cleanup remains open for Product Admin GraphQL writes and status mutations, other ecommerce adapters and non-`PortError` envelopes, and runtime or mounted transport evidence.

## Suggested maintainer checks

```bash
node scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs
node scripts/verify/verify-product-admin-primary-read-error-safety.mjs
node scripts/verify/verify-product-admin-category-read-error-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
