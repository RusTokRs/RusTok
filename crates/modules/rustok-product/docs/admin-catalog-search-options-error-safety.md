# Product Admin catalog search-options and selected transport

Status: **source-ready / unvalidated**

## Scope

This slice covers the Product Admin category and attribute discovery used by the catalog filters, together with the selected transport, tenant admission, error boundary, and pagination behavior that consumes the results.

## Selected transport contract

The Product Admin catalog uses exactly one transport for a build profile:

- **SSR and hydrate:** call the native Product Admin server function.
- **CSR:** use GraphQL bootstrap, then load categories and product attributes.
- **Failure:** do not retry the same operation through the other transport.

The native and GraphQL paths build the same `ProductCatalogSearchOptions` DTO. Category options include structural categories only. Attribute options include attributes that are filterable and have Product or Both scope. Category labels prefer `path`, then `name`, then `code`.

The native schema-read context and the native catalog list/facet server functions enforce the Product module-enabled gate for the current tenant. Native list and facet reads use the trusted request correlation ID and channel when building the Product read-port context.

## Public error boundary

The crate-root export now points to `catalog_transport::fetch_catalog_search_options`. The underlying `transport::fetch_catalog_search_options` executor is crate-private, so external hosts cannot bypass the bounded public wrapper.

The wrapper keeps the public message:

```text
Product catalog search options are temporarily unavailable
```

Its diagnostics record only bounded facts: selected transport, correlation-ID presence/length, token presence, tenant-slug length, locale length, and raw-error presence/length. The complete native or GraphQL error text is not logged or returned to the public UI.

## GraphQL/native parity

The GraphQL ProductCatalogReadPort uses the trusted RequestContext correlation ID rather than a deterministic operation/page string. The native ProductCatalogReadPort uses the same RequestContext correlation ID and channel. Native port errors use the same public error codes and messages as the GraphQL owner mapper, with a request reference in the ServerFnError envelope.

The optional native facet request no longer silently discards failures: it records bounded selected-transport diagnostics before degrading to no facets, matching the GraphQL optional-facet behavior.

## Next.js catalog behavior

The Next.js product page now resolves locale using the host's `EFFECTIVE_LOCALE_HEADER`, `LOCALE_COOKIE`, `locales`, and `defaultLocale` configuration.

After the server returns the catalog total, the route clamps an out-of-range page to the last valid page (or page 1 for an empty catalog), preserving the active filters and page size in the redirect. This avoids rendering an empty page such as page 99 of 2.

The Leptos quick search and local column filters no longer refetch the same server page on every keystroke. They only request page 1 when the current page actually needs to be reset.

## Remaining scope

Schema-authoring mutations still have a native-first/GraphQL-fallback adapter and need a separate selected-transport cutover. Product detail, pricing, and shipping-profile reads remain GraphQL-only. This slice does not claim full Product Admin transport completion.

## Validation

Tests, compilation, formatting, focused verifiers, browser behavior, and mounted SSR/hydrate behavior were not run. They remain for the maintainer to execute.

Suggested checks:

```bash
node scripts/verify/verify-product-admin-catalog-options-error-safety.mjs
node scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs
node scripts/verify/verify-product-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-admin
cargo check -p rustok-product-admin --features hydrate
cargo check -p rustok-product-admin --features ssr
```
