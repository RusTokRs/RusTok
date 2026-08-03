# Product Admin primary GraphQL mutation error safety

Status: **source-ready / unvalidated**

## Scope

This slice covers the public and private diagnostic envelopes for four Product Admin mutations:

- `create_product`;
- `update_product`;
- `change_product_status`;
- `delete_product`.

The boundary lives in:

- `crates/rustok-product/admin/src/catalog_transport.rs`;
- `crates/rustok-product/admin/src/transport/graphql_error_safety.rs`.

The GraphQL mutation documents, variables, input builders, response DTOs and Product owner policy are unchanged.

## Confirmed diagnostic gap

The four operations already used a Product-owned public mapper. `Http(String)` and
`Graphql(String)` were replaced with static Product Admin messages before crossing the facade.

The same mapper still emitted `raw_error = ?error` in private structured tracing. A backend-controlled
HTTP status or GraphQL server message could therefore be copied in full into the diagnostic event even
though the public envelope was safe.

## Boundary placement

A `GraphqlMutationContext` remains created before each selected compatibility transport call. The
wrapper still maps only a final returned `GraphqlHttpError`.

No retry, fallback, extra owner call, mutation document, variable, response mapping or public type is
introduced.

## Public policy

The result error type and messages remain unchanged.

| Captured condition | Public error |
| --- | --- |
| Network failure | `Network error` |
| HTTP failure | `Http error: Product admin service is temporarily unavailable` |
| Unauthorized | `Unauthorized` |
| GraphQL rejection | `GraphQL error: Product admin request could not be completed` |

`Network` and `Unauthorized` remain fixed non-identifying variants. HTTP status text and GraphQL server
messages remain absent from the public result.

## Internal diagnostics

The mapper preserves typed variant classification, stable code, technical-versus-ordinary severity and
correlation. It no longer writes the complete `GraphqlHttpError` Debug representation.

For `Http` and `Graphql`, tracing retains only payload presence and character length. `Network` and
`Unauthorized` retain no invented payload. Each event also records:

- owner, operation and boundary;
- a unique correlation ID;
- token presence, never the token value;
- tenant-slug presence and length;
- tenant-ID and actor-ID lengths;
- product/resource-ID presence and length;
- status presence and length;
- whether a product draft was supplied;
- error kind and stable code.

Raw token, tenant slug, tenant ID, actor ID, product ID, status, draft content and complete backend error
text are not structured fields. The complete typed error is not logged.

## Preserved behavior

This slice does not change:

- `ProductDetail` or boolean mutation result types;
- `CreateProductInput` or `UpdateProductInput` construction;
- tenant/user GraphQL variables;
- status-only update behavior;
- delete-product variables;
- UI save, status and delete command composition;
- public error messages;
- typed error classification or log severity;
- retries or fallback behavior;
- previously merged Product Admin read diagnostic policies;
- Product FFA/FBA, browser, mounted-runtime, workflow, CI or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-primary-graphql-mutation-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-primary-graphql-mutation-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs`.

All execution flags remain `false`. Source review does not prove compilation, browser behavior, mounted
transport behavior, workflow execution, CI or production behavior.

## Remaining work

The ecommerce mapper cleanup remains open for:

- Product Admin native-first GraphQL fallback mutation diagnostics;
- Product Admin browser and mounted transport evidence;
- other ecommerce adapters and non-`PortError` public envelopes.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
