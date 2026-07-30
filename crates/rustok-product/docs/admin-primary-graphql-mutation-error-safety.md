# Product Admin primary GraphQL mutation error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the final public error envelope for four Product Admin mutations:

- `create_product`;
- `update_product`;
- `change_product_status`;
- `delete_product`.

The boundary lives in:

- `crates/rustok-product/admin/src/catalog_transport.rs`;
- `crates/rustok-product/admin/src/transport/graphql_error_safety.rs`.

The GraphQL mutation documents, variables, input builders, response DTOs and Product owner policy are unchanged.

## Confirmed gap

The four primary product commands delegated directly through the compatibility transport to the GraphQL adapter.

`GraphqlHttpError::Http(String)` can contain HTTP status text and
`GraphqlHttpError::Graphql(String)` can contain the first GraphQL server message. Those payloads could therefore reach Product Admin UI command errors.

## Boundary placement

A `GraphqlMutationContext` is created before each selected compatibility transport call. The wrapper maps only a final returned `GraphqlHttpError`.

No retry, fallback, extra owner call, mutation document, variable or response mapping is introduced.

## Public policy

The result error type remains `GraphqlHttpError`.

| Captured condition | Public error |
| --- | --- |
| Network failure | `Network error` |
| HTTP failure | `Http error: Product admin service is temporarily unavailable` |
| Unauthorized | `Unauthorized` |
| GraphQL rejection | `GraphQL error: Product admin request could not be completed` |

`Network` and `Unauthorized` remain fixed non-identifying variants. HTTP status text and GraphQL server messages are replaced with static Product Admin messages.

## Internal diagnostics

The original typed error remains available only to private tracing. Each event records:

- owner, operation and boundary;
- a unique correlation ID;
- token presence, never the token value;
- tenant-slug presence and length;
- tenant-ID and actor-ID lengths;
- product/resource-ID presence and length;
- status presence and length;
- whether a product draft was supplied;
- error kind and stable code.

Raw token, tenant slug, tenant ID, actor ID, product ID, status and draft content are not structured fields.

## Preserved behavior

This slice does not change:

- `ProductDetail` or boolean mutation result types;
- `CreateProductInput` or `UpdateProductInput` construction;
- tenant/user GraphQL variables;
- status-only update behavior;
- delete-product variables;
- UI save, status and delete command composition;
- retries or fallback behavior;
- previously merged Product Admin read error policies;
- Product FFA/FBA, browser, mounted-runtime, workflow, CI or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-primary-graphql-mutation-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-primary-graphql-mutation-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs`.

All execution flags remain `false`. Source review does not prove compilation, browser behavior, mounted transport behavior, workflow execution, CI or production behavior.

## Remaining work

The ecommerce mapper cleanup remains open for:

- Product Admin category, schema and attribute-value GraphQL mutations;
- Product Admin browser and mounted transport evidence;
- other ecommerce adapters and non-`PortError` public envelopes.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-category-read-error-safety.mjs
node scripts/verify/verify-product-admin-primary-read-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
