# Commerce storefront aggregate public error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the Commerce-owned aggregate read:

- `fetch_storefront_commerce`;
- `crates/rustok-commerce/storefront/src/transport/mod.rs`;
- `crates/rustok-commerce/storefront/src/transport/aggregate_error_safety.rs`.

It does not change the payment-collection, shipping-selection, or checkout-completion command wrappers. Those wrappers still use the generic `From<UiTransportError>` conversion and remain explicit follow-up work under the broad ecommerce mapper-cleanup item.

## Confirmed gap

The native and GraphQL aggregate adapters already returned bounded `ApiError` values. Cart and Payment owner failures were mapped to static public messages inside the aggregate composition.

The final Commerce facade nevertheless called:

```rust
.map_err(ApiError::from)
```

The generic conversion used `UiTransportError::to_string()`. Its public value includes transport-path metadata and the nested path error. The aggregate fetch therefore republished the entire final transport display through `ApiError::ServerFn` or `ApiError::Graphql` even though the underlying owner failures had already been sanitized.

## Implementation

`AggregateFetchErrorContext` is created before either transport closure runs. Each call receives a unique correlation id in the namespace:

```text
commerce-storefront-aggregate:fetch_storefront_commerce:<uuid>
```

After `execute_selected_transport`, the final `UiTransportError` is mapped by the Commerce-owned context instead of the generic `ApiError::from` implementation.

### Public policy

| Condition | Public `ApiError` |
| --- | --- |
| Native or GraphQL cart-selection validation | `Validation("Invalid cart selection")` |
| Native aggregate failure | `ServerFn("Storefront commerce data is temporarily unavailable")` |
| GraphQL aggregate failure | `Graphql("Storefront commerce data is temporarily unavailable")` |

The validation classifier accepts the two existing bounded compatibility envelopes:

- `Invalid cart selection` from the native aggregate server function;
- `cart_id must be a valid UUID` from the GraphQL/shared aggregate path.

No arbitrary nested transport text is forwarded to the public result.

## Internal diagnostics

The original `UiTransportError` is retained only as an internal structured tracing field. The event also records:

- owner and owner operation;
- unique correlation id;
- whether a tenant slug is configured and its character length;
- whether a selected cart id and locale are present and their character lengths;
- failed transport path;
- whether fallback was attempted;
- stable internal code;
- boundary name.

The structured request fields do not contain the tenant slug, selected cart id, or locale values. The private error field may retain transport details for operators but is never copied into the public `ApiError`.

## Preserved behavior

This slice does not change:

- `FetchCommerceRequest`;
- `StorefrontCommerceData` and checkout DTOs;
- the private GraphQL aggregate adapter;
- the native aggregate server function;
- Cart and Payment owner requests or response mapping;
- native versus GraphQL feature selection;
- fallback behavior;
- payment collection creation;
- shipping option selection;
- checkout completion;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

Focused source evidence:

- `crates/rustok-commerce/contracts/evidence/storefront-aggregate-error-safety-source.json`;
- `crates/rustok-commerce/contracts/evidence/storefront-aggregate-error-safety-source-review.json`;
- `scripts/verify/verify-commerce-storefront-aggregate-error-safety.mjs`.

The focused verifier is imported by:

- `scripts/verify/verify-commerce-storefront-transport-error-safety.mjs`.

All execution and runtime validation flags remain `false`. Source review alone does not prove native, GraphQL, browser, mounted, workflow, CI, or production behavior.

## Remaining work

The master correlation-safe mapper cleanup remains open for:

- `create_storefront_payment_collection` final Commerce public envelope;
- `select_storefront_shipping_option` final Commerce public envelope;
- `complete_storefront_checkout` final Commerce public envelope;
- Pricing storefront and other remaining ecommerce adapters;
- remaining non-`PortError` public envelopes.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-aggregate-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-handoff.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce-storefront
cargo check -p rustok-commerce-storefront --features hydrate
cargo check -p rustok-commerce-storefront --features ssr
```
