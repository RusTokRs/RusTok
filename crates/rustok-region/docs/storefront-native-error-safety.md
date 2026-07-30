# Region storefront native error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Region-owned native storefront server function in:

- `crates/rustok-region/storefront/src/transport/native_server_adapter.rs`.

The endpoint loads tenant regions, resolves locale fallback, maps country tax policy data, and selects the active region for the storefront response.

## Delivered source contract

Static public `ServerFnError` messages now cover:

- tenant-context extraction failure;
- Region owner service failure while listing storefront regions.

Internal causes remain only in SSR diagnostics with the available:

- Region storefront owner and exact owner operation;
- correlation id, tenant id, channel id, channel slug, and locale when `RequestContext` is available;
- stable internal code and native boundary;
- original extraction or owner-service error.

`RequestContext` remains optional. Extraction failure is logged and still falls back to `None`.

## Preserved behavior

This slice does not change:

- the `region/storefront-data` endpoint;
- `StorefrontRegionsData`, `StorefrontRegion`, or country tax-policy DTOs;
- the external `ApiError` conversion;
- transport selection or GraphQL behavior;
- requested locale precedence: explicit request, request context, tenant default;
- tenant-default locale fallback passed to `RegionService::list_regions`;
- selected-region resolution through `resolve_storefront_regions`;
- region, country, tax-rate, tax-inclusion, or currency mapping.

## Static public messages

- `Region storefront context is unavailable`
- `Storefront regions are temporarily unavailable`

## Static evidence

`scripts/verify/verify-region-storefront-native-error-safety.mjs` guards:

- SSR tracing dependency composition;
- endpoint and host runtime composition;
- optional request-context behavior and diagnostics;
- static tenant-context and owner-service envelopes;
- owner operation, correlation, tenant, channel, locale, stable code, and boundary logging;
- unchanged locale precedence, service inputs, selected-region resolution, country tax-policy mapping, DTO result composition, and outer `ApiError` conversion;
- source-only validation flags.

## Remaining gaps

Compilation, mounted parity, native runtime, remote transport, and browser evidence remain open. The broader ecommerce mapper-cleanup task also remains open for inventory, customer, tax, promotion, compensation/execution adapters, and non-`PortError` public envelopes.

This slice does not change Product dependency topology or make marketplace financial integration an optional Commerce capability.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-region-storefront-native-error-safety.mjs
node scripts/verify/verify-region-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-region-storefront --all-features
```
