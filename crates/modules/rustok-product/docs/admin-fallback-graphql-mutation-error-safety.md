# Product Admin fallback GraphQL mutation error safety

Status: **source-ready / unvalidated**

## Scope

This source slice covers the eleven Product Admin commands that retain a native-first
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

Primary product create, update, status and delete mutations are covered by the separate
primary-mutation policy. Primary reads, category reads and catalog search options retain their
previously merged bounded diagnostic policies.

## Confirmed diagnostic gap

Before this slice, `GraphqlFallbackMutationContext::map_error` returned the typed error unchanged, so a
raw `Http(String)` or `Graphql(String)` payload reached the caller, and it wrote the complete typed
error through `raw_error = ?error` in both tracing severity branches. HTTP status text or a GraphQL
server message could therefore reach both the public result and structured diagnostics.

This slice applies the same classification as the primary read path: the public envelope is static
and the payload is reduced to presence and length before tracing.

## Boundary placement

`catalog_transport.rs` keeps the legacy glob for unchanged functions and explicitly re-exports the
eleven sanitized wrappers. The named exports take precedence over the glob without deleting or
rewriting compatibility executors.

Each wrapper:

1. creates one correlation context before invoking the compatibility executor;
2. invokes that executor exactly once;
3. maps only a final returned `GraphqlHttpError`;
4. preserves the original result type and successful response;
5. adds no retry, native call, GraphQL call or fallback.

The compatibility executor remains responsible for native-first selection. A final wrapper error
therefore means the native path failed and the single GraphQL fallback also failed.

## Public error policy

The public type and the four static messages below are the target policy.

| Internal variant | Public result |
| --- | --- |
| `Network` | `Network` (`Network error`) |
| `Http(raw)` | `Http("Product admin service is temporarily unavailable")` |
| `Unauthorized` | `Unauthorized` |
| `Graphql(raw)` | `Graphql("Product admin request could not be completed")` |

## Correlation-safe diagnostics

The complete typed error is not logged. The mapper still matches the typed variant before building
the public result, severity and stable code. For `Http` and `Graphql` variants, tracing retains
only payload presence and character length. `Network` and `Unauthorized` retain no invented payload.

Every final fallback failure also records:

- owner, operation, stable code, boundary and error classification;
- a UUID-based correlation identifier in the `product-admin-fallback-mutation` namespace;
- token and tenant-slug presence;
- tenant, actor, resource and locale character lengths;
- patch or attribute-ID collection counts where applicable;
- input presence and native-fallback state.

The logger does not emit token, tenant slug, tenant ID, actor ID, product ID, locale, draft
contents, patch contents, attribute identifiers, HTTP status text or GraphQL server messages as
structured values.

## Schema-write wire contract (changed in the same wave)

The eleven schema-authoring documents now live in `transport/product_schema_graphql.rs`. Each declares a
non-null `$idempotencyKey: String!` and forwards it. The `tenantId`/`userId` variables were removed: the
mounted resolvers take no tenant or user arguments and derive the actor from the request context. The
retry identity is in `schema_retry_identity.rs`.

The native-first path carries the same caller key as the GraphQL fallback. `transport.rs` mints one key per
logical invocation through `run_keyed_schema_write`; the native `#[server]` function builds a `PortContext`
with that key, and the owner derives its receipt key with `rustok_product::scoped_caller_idempotency_key`
(the formula used by the mounted Commerce resolvers). A native write whose response is lost is therefore
deduplicated when the fallback retries with the same key.

A GraphQL response without a confirmed `true` (null or `false`, without a GraphQL error) is a protocol error,
not success: the key stays retained for an explicit retry.

## Preserved transport behavior

This slice does not change:

- the native-first / GraphQL-fallback order;
- the number of GraphQL fallback attempts;
- mutation input DTOs or normalization;
- boolean and attribute-value response mapping;
- UI action composition;
- retries, timeouts or transport selection;
- Product FFA/FBA, browser, mounted-runtime, workflow, CI or production status.

## Source evidence

The source contract and review are recorded in:

- `contracts/evidence/admin-fallback-graphql-mutation-error-safety-source.json`
- `contracts/evidence/admin-fallback-graphql-mutation-error-safety-source-review.json`

The focused fail-closed guard is:

- `scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs`

The guard preserves the original facade, wrapper, native-first, GraphQL document, variable, result,
input-mapping and prior-policy checks while also failing closed against complete fallback error
payload logging.

## Validation status

No repository tests, Node verifiers, Cargo commands, formatting, workflows, CI, browser execution
or mounted transport execution were run for this source slice. The evidence intentionally remains
unvalidated.

Suggested maintainer execution:

```bash
node scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs
node scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```

## Remaining work

The known Product Admin catalog-options, read, primary mutation and fallback mutation GraphQL
diagnostic envelopes no longer intentionally retain complete backend error payloads. Product Admin
browser and mounted execution evidence remain open, as does the broader ecommerce cleanup for
non-`PortError` public and diagnostic envelopes in other owners.
