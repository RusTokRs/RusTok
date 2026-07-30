# Product Admin category GraphQL read error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens five category-bound Product Admin reads:

- `fetch_product_attributes`;
- `fetch_catalog_categories`;
- `fetch_attribute_schemas`;
- `fetch_effective_product_form`;
- `fetch_product_attribute_values`.

These operations keep the existing native server function as their first path and use GraphQL only as the public/headless fallback.

## Confirmed gap

The native-first executor already swallowed a native transport failure and then returned the result of the existing GraphQL fallback. Before this slice, a final fallback failure crossed the Product Admin facade as `GraphqlHttpError` without the Product-owned public mapper introduced for the primary reads.

Two variants carry backend-controlled detail:

- `GraphqlHttpError::Http(String)` can contain HTTP status text;
- `GraphqlHttpError::Graphql(String)` can contain the first GraphQL server message.

That detail could reach Leptos resource error handling.

## Boundary placement

The final wrappers live in `crates/rustok-product/admin/src/catalog_transport.rs`.

Each wrapper creates one `GraphqlReadContext` before invoking the unchanged native-first executor in `transport.rs`. The mapper is called only when the executor returns an error, which means the native path failed and the GraphQL fallback also failed.

This placement preserves the existing executor without duplicating its owner call or fallback selection.

## Public policy

The result type remains `GraphqlHttpError`.

| Captured condition | Public error |
| --- | --- |
| Network failure | `Network error` |
| HTTP failure | `Http error: Product admin service is temporarily unavailable` |
| Unauthorized | `Unauthorized` |
| GraphQL rejection | `GraphQL error: Product admin request could not be completed` |

`Network` and `Unauthorized` were already static variants. HTTP status text and GraphQL server messages are replaced with fixed Product Admin messages.

## Internal diagnostics

The private tracing event retains the original typed error and records:

- owner, operation, stable code, boundary, and correlation ID;
- token presence, never the token value;
- tenant-slug, tenant-ID, resource-ID, category-ID, and locale presence/length;
- whether the native fallback was attempted;
- classified error kind.

For effective-form reads, product and category identifiers use separate shape fields. No token, tenant slug, tenant ID, product ID, category ID, or locale value is emitted as a structured field.

## Preserved behavior

This slice does not change:

- the native-first executor;
- the GraphQL fallback order;
- query documents or variables;
- response DTO mapping;
- result types;
- effective-form product/category selection;
- attribute-value request semantics;
- retries or fallback count;
- primary-read and catalog search-options policies;
- FFA, FBA, browser, mounted-runtime, workflow, CI, or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-category-read-error-safety.mjs`.

All execution flags remain `false`. Source review does not prove compilation, mounted transport behavior, browser behavior, workflow execution, CI, or production behavior.

## Remaining work

The broad ecommerce mapper cleanup remains open for:

- Product Admin GraphQL writes and status mutations;
- other ecommerce adapters and non-`PortError` public envelopes;
- runtime and mounted transport evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-product-admin-category-read-error-safety.mjs
node scripts/verify/verify-product-admin-primary-read-error-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
