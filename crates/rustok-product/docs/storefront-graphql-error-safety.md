# Product storefront GraphQL error safety

Status: source-unvalidated

## Scope

This source slice hardens both the public and private diagnostic boundary for the
product-owned storefront GraphQL transport used by two public operations:

- `fetch_products`;
- `fetch_catalog_search_options`.

The storefront facade remains the public transport consumer. The low-level
GraphQL adapter remains private and continues to own the catalog-list, selected
product, pricing-product, and catalog-search-options documents, variables,
response decoding, request ordering, and DTO mapping.

## Rechecked gap

The public consumer already created a `GraphqlCallContext`, reparsed each private
adapter display handoff through `GraphqlHttpError::from_str`, and returned static
product-owned messages. GraphQL server messages and HTTP/client detail therefore
did not cross the public storefront envelope.

The shared structured event still copied two complete private values:

- `raw_error = %raw_error`;
- `parsed_error = ?parsed_error`.

Those payloads were unnecessary for correlation, exact owner-operation
attribution, or the closed five-category transport policy. This was a remaining
non-`PortError` diagnostic-envelope gap in the ecommerce correlation-safe mapper
cleanup.

## Public consumer policy

`transport/mod.rs` creates a private `GraphqlCallContext` before invoking each
GraphQL adapter operation. The context reparses the adapter's display handoff
through `GraphqlHttpError::from_str` and maps only `ApiError::Graphql`.

The stable public messages remain:

| Typed failure | Public message |
| --- | --- |
| network | `Product storefront is temporarily unavailable` |
| HTTP | `Product storefront is temporarily unavailable` |
| unauthorized | `Product storefront authentication is required` |
| GraphQL rejection | `Product storefront request could not be completed` |
| unknown display envelope | `Product storefront request could not be completed` |

`ApiError::ServerFn` is returned unchanged. This preserves the independent
native transport policy.

## Correlation and bounded diagnostics

Every selected GraphQL call creates a unique correlation identifier using the
owner operation and a new UUID. Internal tracing events retain only:

- owner `rustok_product.storefront`;
- operation `fetch_products` or `fetch_catalog_search_options`;
- boundary `product_storefront_graphql_transport`;
- correlation identifier;
- one closed error category: `network`, `http`, `unauthorized`, `graphql`, or
  `unknown`;
- stable internal code;
- raw-display presence and character length;
- whether typed `GraphqlHttpError` parsing succeeded;
- whether a tenant slug is configured and its character length;
- optional selected-handle, locale, currency, region, price-list, channel-id,
  channel-slug, search, and category-id presence and character lengths;
- quantity presence without its value;
- sort-field and sort-direction presence;
- attribute-filter count.

Raw GraphQL display text is not written to the event.
Debug output from the parsed typed error is not written to the event.

The mapper also does not log raw tenant slug, selected handle, locale, currency,
region id, price-list id, channel id, channel slug, quantity, search text,
category id, sort values, attribute-filter values, GraphQL endpoint, documents,
variables, tokens, or returned product data.

Technical network, HTTP, and unknown failures continue to use error-level
diagnostics. Unauthorized and ordinary GraphQL rejection continue to use
warning-level diagnostics.

## Covered operations

`fetch_products` retains its existing catalog-list, optional selected-product,
and optional pricing request sequence under one owner context.

`fetch_catalog_search_options` retains its independent owner context and the
existing catalog-search-options request.

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
- native error-safety policy;
- static public messages, stable codes, category severity, or non-GraphQL
  pass-through behavior.

The adapter still creates the exact typed GraphQL display handoff. Sanitization
occurs once at the public consumer boundary, avoiding duplicate diagnostics and
preserving adapter ownership.

## Source guard

The focused verifier is:

```bash
node scripts/verify/verify-product-storefront-graphql-error-safety.mjs
```

It checks the typed mapping policy, all five static public categories, stable
codes, correlation and bounded request-shape diagnostics, absence of raw GraphQL
and parsed Debug payloads, GraphQL-only remapping, private-adapter preservation,
native-path preservation, truthful source/review evidence, and validation
nonclaims.

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
