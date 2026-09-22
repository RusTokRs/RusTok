# GraphQL checkout service diagnostic safety

Status: `source_hardened_unvalidated`

## Scope

This source wave continues the canonical ecommerce correlation-safe mapper cleanup at the Commerce GraphQL checkout shipping-service boundary.

The private `safe_checkout.rs` facade still owns the eight unchanged checkout resolver service calls:

- four shipping-option calls delegated to Fulfillment;
- four shipping-profile calls delegated to Commerce.

The resolver source, mutation signatures, permission checks, request fields, owner calls, result DTOs, and existing GraphQL errors remain unchanged.

## Public policy preserved

Shipping profiles retain:

- `SHIPPING_PROFILE_REQUEST_INVALID`;
- `SHIPPING_PROFILE_NOT_FOUND`;
- `SHIPPING_PROFILE_STATE_CONFLICT`;
- `SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE`;
- `SHIPPING_PROFILE_OPERATION_FAILED`.

Shipping options retain:

- `SHIPPING_OPTION_REQUEST_INVALID`;
- `SHIPPING_OPTION_NOT_FOUND`;
- `SHIPPING_OPTION_STATE_CONFLICT`;
- `SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE`;
- `SHIPPING_OPTION_OPERATION_FAILED`.

Only the existing database envelopes remain retryable. Already-constructed `async_graphql::Error` values continue to pass through unchanged.

## Diagnostic policy

Complete `CommerceError` and `FulfillmentError` values are no longer formatted into tracing events. Both owner mappers consume their source structurally, classify it through the existing exhaustive match, and emit only:

- a zero-sized diagnostic token whose `Debug` representation is `redacted`;
- the static owner name;
- the stable classified error kind;
- the public code and retryability;
- the static checkout boundary and event message.

Validation text, UUIDs, slugs, transition values, database causes, rich/core details, and inventory state remain outside diagnostics and public envelopes.

## Deliberate limits

This slice does not change checkout orchestration, owner services, public GraphQL schema, resolver source, severity policy, other Commerce adapters, native/REST transports, or FFA/FBA status. The broader ecommerce correlation-safe mapper cleanup remains open.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-graphql-checkout-service-error-safety.mjs
cargo check -p rustok-commerce --lib
```

Tests, Node verifiers, Cargo commands, formatting, mounted GraphQL execution, workflows, and CI were not run for this source wave.