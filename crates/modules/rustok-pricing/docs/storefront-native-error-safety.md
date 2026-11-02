# Pricing storefront native error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Pricing-owned native storefront server function in:

- `crates/modules/rustok-pricing/storefront/src/transport/native_server_adapter.rs`.

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
- correlation id, tenant id, channel id, channel slug, and locale from the required trusted `RequestContext`;
- stable internal code and native boundary;
- original runtime, extraction, service, or projection error.

Trusted `RequestContext` is required. An extraction failure is logged as a closed
context rejection and returns a static public error; it never widens into a `None`
channel/default read.

## Preserved behavior

The 2026-10-29 quote continuation preserves:

- the `pricing/storefront-data` endpoint;
- the external `ApiError::ServerFn` transport variant;
- build-profile-selected transport without runtime fallback;
- explicit channel-id validation;
- resolution-context validation;
- locale fallback to the tenant default locale;
- channel listing pagination at page `1`, per-page `250`;
- product pricing pagination at page `1`, per-page `8`;
- selected-handle fallback behavior.

The two transport-validation errors intentionally remain user-facing. Runtime, context, owner-service, and internal projection errors use static envelopes.

## Static evidence

`scripts/verify/verify-pricing-storefront-native-error-safety.mjs` guards:

- SSR tracing dependency composition;
- the endpoint and external error variant;
- required trusted request-context behavior and diagnostics;
- static runtime, context, owner-service, and projection envelopes;
- owner operation, correlation, tenant, channel, locale, stable code, and boundary logging;
- preservation of exactly two transport-validation mappings;
- paired channel selection, owner read-port composition, snapshot/source/context/time DTOs and preserved pagination;
- source-only validation flags.

## Remaining gaps

Compile, mounted parity, native runtime, remote transport, and browser evidence remain open. The broader ecommerce mapper-cleanup task also remains open for other transports, compensation/execution adapters, and non-`PortError` public envelopes.

Pricing now declares its existing Region owner collaboration as a runtime dependency; this does not move region lifecycle or taxes into Pricing.

## Suggested maintainer checks

The focused native/aggregate source guards were run and passed for the quote continuation. Broader runtime checks below remain with the maintainer:

```bash
node scripts/verify/verify-pricing-storefront-native-error-safety.mjs
node scripts/verify/verify-pricing-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing-storefront --all-features
```


## Owner quote continuation (2026-10-29)

Channel options remain Channel-owned. Active lists, real atlas coverage, public detail
and bounded effective-price batches now use the canonical Pricing read port. The
native adapter retains the request correlation and effective locale; failure maps
through static owner presentation. Explicit ID-only/slug-only selection is not mixed
with the other half of a host channel. Native and GraphQL carry the actual price-row
source, normalized context and one resolution time. No source check proves runtime
execution, database isolation or mounted parity. No tests, Cargo commands, workflows
or CI were run; non-test checks are recorded in the Pricing implementation plan.
