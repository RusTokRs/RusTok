# Product Admin category GraphQL read error safety

Status: **source-ready / unvalidated**

## Scope

This contract covers five category-bound Product Admin reads:

- `fetch_product_attributes`;
- `fetch_catalog_categories`;
- `fetch_attribute_schemas`;
- `fetch_effective_product_form`;
- `fetch_product_attribute_values`.

These operations retain their native-first executor and use GraphQL only as the existing public/headless fallback.

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

The category wrappers already called the shared static public mapper after the native-first executor returned a final GraphQL failure. The shared read mapper nevertheless logged `raw_error = ?error`, retaining the complete backend-controlled payload in structured tracing.

The mapper now records only payload presence and character length for `Http` and `Graphql`. `Network` and `Unauthorized` retain no invented payload. The complete typed error is not logged.

The event still retains:

- owner, operation, stable code, boundary, and closed error kind;
- a unique correlation ID;
- token presence and bounded tenant, product, category, and locale shape;
- whether native fallback was attempted;
- technical-error versus ordinary-rejection severity.

For effective-form reads, product and category identifiers remain separate shape fields. Raw request values, HTTP text, GraphQL messages, and complete Debug error payloads are not structured fields.

## Preserved behavior

This change does not alter:

- native server-function-first execution;
- GraphQL fallback order or count;
- query documents, variables, or response mapping;
- category-bound result types;
- effective-form product/category selection;
- attribute-value request semantics;
- primary-read and catalog search-options public policies;
- retries or owner calls;
- Product FFA/FBA, browser, mounted-runtime, workflow, CI, or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs`;
- compatibility command `scripts/verify/verify-product-admin-category-read-error-safety.mjs`.

All execution flags remain `false`. Source review does not prove compilation, verifier execution, mounted transport behavior, browser behavior, workflow execution, CI, or production behavior.

## Remaining work

The broad ecommerce mapper cleanup remains open for Product Admin GraphQL writes and status mutations, other ecommerce adapters and non-`PortError` public envelopes, and runtime or mounted transport evidence.

## Suggested maintainer checks

```bash
node scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs
node scripts/verify/verify-product-admin-category-read-error-safety.mjs
node scripts/verify/verify-product-admin-primary-read-error-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
