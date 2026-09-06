# Product storefront catalog native error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Product-owned native catalog-list server-function adapter in:

- `crates/rustok-product/storefront/src/transport/catalog_list_native.rs`.

It covers missing host-composed `TransactionalEventBus`, optional request-context diagnostics, and tenant-context extraction. Product catalog query validation and service failures continue to use the existing Product public-error mapper.

## Delivered source contract

The native adapter now returns static public messages for:

- missing `TransactionalEventBus` runtime composition;
- tenant-context extraction failure.

Internal context failures remain only in SSR diagnostics with the available:

- Product storefront owner and exact operation;
- correlation id, channel id, channel slug, and locale when optional `RequestContext` is available;
- stable internal code and native boundary;
- original context extraction error.

`RequestContext` remains optional. Extraction failure is logged and still falls back to `None`, preserving locale and channel fallback behavior.

## Preserved behavior

This slice does not change:

- the `product/storefront/catalog-list` endpoint;
- `CatalogListInput`, `FetchRequest`, `ProductList`, or `StorefrontProductsData`;
- selected-handle fallback;
- `StorefrontProductListQuery::try_from_transport_with_attribute_filters`;
- pagination at page `1`, per-page `12`;
- locale fallback to the tenant default locale;
- request-context channel-slug fallback;
- Product `map_product_public_error` handling for input and catalog service failures;
- Product dependency topology or lifecycle collaboration contracts.

## Static evidence

`scripts/verify/verify-product-storefront-catalog-native-error-safety.mjs` guards:

- existing SSR tracing composition;
- exact endpoint, operation, query, pagination, locale, and channel markers;
- static runtime and tenant-context public envelopes;
- optional request-context preservation and diagnostics;
- correlation, channel, locale, owner, code, and boundary logging;
- removal of raw runtime/context public mappings;
- unchanged Product public-error mapper calls;
- source-only validation flags.

## Remaining gaps

The Product dependency contract remains unresolved. This slice does not change Product/Inventory/Pricing topology, lifecycle transactions, host composition, or module enablement semantics.

The broader ecommerce mapper-cleanup task also remains open for remaining transports, compensation/execution adapters, and non-`PortError` public envelopes. Compile, mounted parity, remote transport, and runtime evidence remain open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-product-storefront-catalog-native-error-safety.mjs
node scripts/verify/verify-product-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-product-storefront --all-features
```
