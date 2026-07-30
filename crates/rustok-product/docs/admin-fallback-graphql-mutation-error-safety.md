# Product Admin fallback GraphQL mutation error safety

Status: **source-ready / unvalidated**

## Scope

This source slice covers the eleven Product Admin commands that retain a native
server-function path and use GraphQL only as the compatibility fallback:

- `create_product_attribute`
- `create_product_attribute_option`
- `create_catalog_category`
- `create_attribute_schema`
- `set_category_schema_mode`
- `create_product_attribute_schema_group`
- `create_category_attribute_group`
- `bind_schema_attribute`
- `bind_category_attribute`
- `save_product_attribute_values`
- `clear_detached_product_attribute_values`

Primary product create, update, status, and delete mutations are covered by the
separate primary-mutation policy. Primary reads, category reads, and catalog
search options also retain their previously merged policies.

## Confirmed boundary gap

The compatibility executors already attempted native server functions first.
When native execution failed, they delegated once to the existing GraphQL
mutation. A final `GraphqlHttpError::Http(String)` could carry response status
text and `GraphqlHttpError::Graphql(String)` could carry the first server error
message into Product Admin command handling.

## Boundary placement

`catalog_transport.rs` keeps the legacy glob for all unchanged functions and
explicitly re-exports the eleven sanitized wrappers. The named exports take
precedence over the glob without deleting or rewriting the compatibility
executors.

Each wrapper:

1. creates a correlation context before invoking the compatibility executor;
2. invokes that executor exactly once;
3. maps only a final returned `GraphqlHttpError`;
4. preserves the original result type and successful response;
5. adds no retry, native call, GraphQL call, or fallback.

The compatibility executor remains responsible for the original native-first
selection. Therefore a final wrapper error means the native path failed and the
single GraphQL fallback also failed.

## Public error policy

The public type remains `GraphqlHttpError`.

| Internal variant | Public result |
| --- | --- |
| `Network` | `Network` (`Network error`) |
| `Http(raw)` | `Http("Product admin service is temporarily unavailable")` |
| `Unauthorized` | `Unauthorized` |
| `Graphql(raw)` | `Graphql("Product admin request could not be completed")` |

HTTP status text and GraphQL server messages remain available only in private
structured diagnostics.

## Correlation-safe diagnostics

Every final fallback failure receives a UUID-based correlation identifier in the
`product-admin-fallback-mutation` namespace. Structured fields are limited to:

- owner, operation, stable code, boundary, and error classification;
- token and tenant-slug presence;
- tenant, actor, resource, and locale character lengths;
- patch or attribute-id collection counts where applicable;
- input presence and native-fallback state;
- the original typed error as a private tracing field.

The logger does not emit token, tenant slug, tenant ID, actor ID, product ID,
locale, draft contents, patch contents, or attribute identifiers as structured
values.

## Preserved transport behavior

This slice does not change:

- native server adapter functions;
- the native-first / GraphQL-fallback order;
- the number of GraphQL fallback attempts;
- GraphQL documents or variables;
- tenant/user variable framing;
- mutation input DTOs or normalization;
- boolean and attribute-value response mapping;
- UI action composition;
- retries, timeouts, or transport selection.

## Source evidence

The source contract and review are recorded in:

- `contracts/evidence/admin-fallback-graphql-mutation-error-safety-source.json`
- `contracts/evidence/admin-fallback-graphql-mutation-error-safety-source-review.json`

The focused fail-closed guard is:

- `scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs`

## Validation status

No repository tests, Node verifiers, Cargo commands, formatting, workflows, CI,
browser execution, or mounted transport execution were run for this source
slice. The evidence intentionally remains unvalidated.

Suggested maintainer execution:

```bash
node scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-category-read-error-safety.mjs
node scripts/verify/verify-product-admin-primary-read-error-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```

## Remaining work

The known Product Admin GraphQL transport functions are now source-covered by
bounded policies. Product Admin browser and mounted transport execution evidence
remain open, as does the broader ecommerce cleanup for non-`PortError` public
envelopes in other owners.
