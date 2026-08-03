# Product Admin catalog search-options error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the final public `String` error and its private diagnostic envelope for Product Admin catalog search-option discovery:

- `crates/rustok-product/admin/src/catalog_transport.rs`;
- public `fetch_catalog_search_options` re-exported from the Product Admin crate root.

The existing implementation in `transport.rs` remains the private compatibility executor. It still tries the native owner endpoint first and, only after native failure, performs the GraphQL fallback sequence:

1. load the current tenant bootstrap;
2. load catalog categories;
3. load product attributes;
4. build category and attribute search options.

## Recheck finding

The public boundary was already static and safe. The compatibility executor converted each GraphQL failure with `err.to_string()`, and the crate-root wrapper replaced that value with `Product catalog search options are temporarily unavailable` before returning it to the UI.

The residual gap was private diagnostics: `CatalogSearchOptionsErrorContext::map_error` still wrote the complete captured String through `raw_error = %raw_error`. Depending on the failure, that payload could contain GraphQL server messages, HTTP status text, parse details, or transport classification. The focused verifier and retained evidence explicitly required that complete payload.

## Boundary placement

The crate-root catalog transport continues to own the public wrapper around the unchanged compatibility executor. It creates `CatalogSearchOptionsErrorContext` before the native/GraphQL fallback begins.

When the compatibility executor returns an error, the wrapper now:

- does not write the captured error text to structured tracing;
- records only whether the raw error is present and its character length;
- attaches a unique correlation id;
- records token presence, tenant-slug presence/length, and locale length;
- returns `Product catalog search options are temporarily unavailable`.

The public success DTO and `Result<_, String>` shape remain unchanged.

## Correlation policy

Each request receives an id in the namespace:

```text
product-admin-catalog-options:fetch_catalog_search_options:<uuid>
```

The stable internal code is:

```text
product.admin_catalog_search_options_graphql_unavailable
```

## Data minimization

Structured diagnostics contain only bounded classification and shape facts. They do not contain:

- the complete GraphQL, HTTP, parse, or transport error payload;
- the authentication token;
- tenant slug or tenant id;
- locale value;
- category ids;
- attribute ids or codes;
- option labels;
- GraphQL variables or response data.

The raw error is consumed only to derive presence and character length before the static public message is returned.

## Preserved behavior

This slice does not change:

- the native-first fallback policy;
- the native search-options server function;
- GraphQL bootstrap, category, or attribute documents and variables;
- tenant resolution order;
- category label precedence (`path`, then `name`, then `code`);
- filterable/sortable attribute selection;
- `ProductCatalogSearchOptions` or its public re-export;
- the UI resource call;
- retries, transport selection, or fallback count;
- Product FFA/FBA, browser, mounted, CI, or production status.

## Static evidence

- `crates/rustok-product/contracts/evidence/admin-catalog-search-options-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/admin-catalog-search-options-error-safety-source-review.json`;
- `scripts/verify/verify-product-admin-catalog-options-error-safety.mjs`.

The focused verifier now fails closed if the complete raw error returns to structured tracing. All execution fields remain false. Source review does not prove compilation, verifier execution, browser behavior, or mounted fallback behavior.

## Remaining work

The ecommerce correlation-safe mapper cleanup remains open for other Product Admin GraphQL reads and mutations, other owner adapters, and remaining non-`PortError` envelopes.

## Suggested maintainer checks

```bash
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
