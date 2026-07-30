# Product Admin catalog search-options error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the final public `String` error returned by Product Admin catalog search-option discovery:

- `crates/rustok-product/admin/src/catalog_transport.rs`;
- public `fetch_catalog_search_options` re-exported from the Product Admin crate root.

The existing implementation in `transport.rs` remains the private compatibility executor. It still tries the native owner endpoint first and, only after native failure, performs the GraphQL fallback sequence:

1. load the current tenant bootstrap;
2. load catalog categories;
3. load product attributes;
4. build category and attribute search options.

## Confirmed gap

The compatibility executor converted each GraphQL failure with `err.to_string()` and returned `Result<ProductCatalogSearchOptions, String>`. The UI resource consumed that result directly.

The captured string can include GraphQL server messages, HTTP status text, or transport classification details. Those values are useful for private diagnostics but are not a stable Product Admin public contract.

## Boundary placement

The crate-root catalog transport now owns a public wrapper around the unchanged compatibility executor. It creates `CatalogSearchOptionsErrorContext` before the native/GraphQL fallback begins.

When the compatibility executor returns an error, the wrapper:

- logs the captured string only in private tracing;
- attaches a unique correlation id;
- records only token presence, tenant-slug presence/length, and locale length;
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

Structured diagnostics do not contain:

- the authentication token;
- tenant slug or tenant id;
- locale value;
- category ids;
- attribute ids or codes;
- option labels;
- GraphQL variables or response data.

Only presence and character-length facts are retained alongside the private captured error.

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

All execution fields remain false. Source review does not prove compilation, verifier execution, browser behavior, or mounted fallback behavior.

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
