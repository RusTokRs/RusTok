# Product storefront GraphQL error safety

Status: source-unvalidated

## Scope

This source slice hardens the product-owned storefront GraphQL transport for two
public operations:

- `fetch_products`;
- `fetch_catalog_search_options`.

The storefront facade remains the public transport consumer. The low-level
GraphQL adapter remains private and continues to own the catalog-list, selected
product, pricing-product, and catalog-search-options documents, variables,
response decoding, request ordering, and DTO mapping.

## Previous gap

Both selected GraphQL closures delegated directly to the private adapter. The
adapter converted `GraphqlHttpError` into `ApiError::Graphql(value.to_string())`,
and `execute_selected_transport` retained that display string in the public
`UiTransportError.graphql_error` field.

Consequently, server-provided GraphQL messages and HTTP detail could cross the
storefront UI boundary without a product-owned public-envelope policy. The
native catalog-list and catalog-search-options paths already had their own
owner mappings, but those mappings did not cover the GraphQL path.

## Public consumer policy

`transport/mod.rs` now creates a private `GraphqlCallContext` before invoking
each GraphQL adapter operation. The context reparses the adapter's display
handoff through `GraphqlHttpError::from_str` and maps only `ApiError::Graphql`.

The stable public messages are:

| Typed failure | Public message |
| --- | --- |
| network | `Product storefront is temporarily unavailable` |
| HTTP | `Product storefront is temporarily unavailable` |
| unauthorized | `Product storefront authentication is required` |
| GraphQL rejection | `Product storefront request could not be completed` |
| unknown display envelope | `Product storefront request could not be completed` |

`ApiError::ServerFn` is returned unchanged. This preserves the independent
native transport policy.

## Correlation diagnostics

Every selected GraphQL call creates a unique correlation identifier using the
owner operation and a new UUID. Internal tracing events retain:

- owner `rustok_product.storefront`;
- operation `fetch_products` or `fetch_catalog_search_options`;
- boundary `product_storefront_graphql_transport`;
- correlation identifier;
- typed error kind and stable code;
- raw and reparsed GraphQL cause for internal diagnosis;
- whether a tenant slug is configured and its character length;
- optional selected-handle, locale, currency, region, price-list, channel-id,
  channel-slug, search, and category-id presence and character lengths;
- quantity presence without its value;
- sort-field and sort-direction presence;
- attribute-filter count.

The mapper does not log raw tenant slug, selected handle, locale, currency,
region id, price-list id, channel id, channel slug, quantity, search text,
category id, sort values, attribute-filter values, GraphQL endpoint, documents,
variables, tokens, or returned product data.

Technical network, HTTP, and unknown failures use error-level diagnostics.
Unauthorized and ordinary GraphQL rejection use warning-level diagnostics.

## Preserved behavior

This slice does not change:

- feature-based `NativeServer` versus `Graphql` selection;
- the no-fallback transport policy;
- the private GraphQL adapter;
- `STOREFRONT_PRODUCTS_QUERY`;
- `STOREFRONT_PRODUCT_QUERY`;
- `STOREFRONT_PRICING_PRODUCT_QUERY`;
- `STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY`;
- typed `CatalogListInput` normalization;
- fixed catalog page and page-size request values;
- selected-product and pricing request order;
- product request or response DTOs;
- pricing-context construction;
- native owner catalog-list execution;
- native catalog-search-options server function;
- native error-safety policy.

The adapter still creates the exact typed GraphQL display handoff. Sanitization
occurs once at the public consumer boundary, avoiding duplicate diagnostics and
preserving adapter ownership.

## Source guard

The focused verifier is:

```bash
node scripts/verify/verify-product-storefront-graphql-error-safety.mjs
```

It checks the typed mapping policy, static public messages, stable codes,
correlation and safe request-shape diagnostics, GraphQL-only remapping,
private-adapter preservation, native-path preservation, evidence status, and
validation nonclaims.

The general owner boundary verifier imports the focused guard:

```bash
node scripts/verify/verify-product-storefront-boundary.mjs
```

## Evidence boundary

Retained source evidence:

- `crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source-review.json`.

The evidence establishes only that the source and guardrail are present and
have been reviewed. It does not establish compilation, browser execution,
GraphQL runtime behavior, mounted parity, workflow success, CI success, or
production behavior.

The broad ecommerce correlation-safe mapper cleanup remains open.

No tests, verifiers, Cargo commands, formatting, workflows, or CI were run per
maintainer instruction.

## Suggested maintainer verification

```bash
node scripts/verify/verify-product-storefront-graphql-error-safety.mjs
node scripts/verify/verify-product-storefront-catalog-native-error-safety.mjs
node scripts/verify/verify-product-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-storefront
cargo check -p rustok-product-storefront --features hydrate
cargo check -p rustok-product-storefront --features ssr
```

Source readiness must remain separate from runtime or promotion claims until
those commands and required mounted failure evidence are retained on the same
revision.
