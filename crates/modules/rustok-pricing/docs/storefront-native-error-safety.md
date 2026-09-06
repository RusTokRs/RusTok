# Pricing storefront native error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Pricing-owned native storefront server function in:

- `crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs`.

The endpoint composes channel options, active price lists, published product pricing, selected-product detail, and effective variant prices.

## Delivered source contract

Static public `ServerFnError` messages now cover:

- missing host-composed `TransactionalEventBus`;
- tenant-context extraction failure;
- channel listing failure;
- active price-list loading failure;
- published pricing list failure;
- selected product pricing detail failure;
- internal variant-id projection failure;
- effective variant-price resolution failure.

Internal causes remain only in SSR diagnostics with the available:

- Pricing storefront owner and exact owner operation;
- correlation id, tenant id, channel id, channel slug, and locale when optional `RequestContext` is available;
- stable internal code and native boundary;
- original runtime, extraction, service, or projection error.

`RequestContext` remains optional. Extraction failure is logged and still falls back to `None`.

## Preserved behavior

This slice does not change:

- the `pricing/storefront-data` endpoint;
- `StorefrontPricingQuery`, `StorefrontPricingData`, or nested DTOs;
- the external `ApiError::ServerFn` transport variant;
- transport selection or GraphQL behavior;
- explicit channel-id validation;
- resolution-context validation;
- locale fallback to the tenant default locale;
- request-context channel id and channel slug fallback;
- channel listing pagination at page `1`, per-page `250`;
- product pricing pagination at page `1`, per-page `8`;
- active price-list, product list, product detail, or effective-price operation payloads;
- selected-handle fallback behavior.

The two transport-validation errors intentionally remain user-facing. Runtime, context, owner-service, and internal projection errors use static envelopes.

## Static evidence

`scripts/verify/verify-pricing-storefront-native-error-safety.mjs` guards:

- SSR tracing dependency composition;
- the endpoint and external error variant;
- optional request-context behavior and diagnostics;
- static runtime, context, owner-service, and projection envelopes;
- owner operation, correlation, tenant, channel, locale, stable code, and boundary logging;
- preservation of exactly two transport-validation mappings;
- unchanged locale/channel fallback, resolution context, pagination, service operations, and DTO result composition;
- source-only validation flags.

## Remaining gaps

Compile, mounted parity, native runtime, remote transport, and browser evidence remain open. The broader ecommerce mapper-cleanup task also remains open for other transports, compensation/execution adapters, and non-`PortError` public envelopes.

This slice does not change Product/Pricing dependency topology or make marketplace financial integration an optional Commerce capability.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-pricing-storefront-native-error-safety.mjs
node scripts/verify/verify-pricing-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing-storefront --all-features
```
