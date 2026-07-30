# Pricing storefront GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the Pricing storefront GraphQL public error boundary:

- `crates/rustok-pricing/storefront/src/transport/mod.rs`;
- `crates/rustok-pricing/storefront/src/transport/graphql_error_safety.rs`;
- the final GraphQL branch of `fetch_storefront_pricing`.

The GraphQL adapter query documents, variables, list/detail composition, result mapping,
Pricing query validation, and the native server-function adapter remain unchanged.

## Confirmed gap

The Pricing GraphQL adapter intentionally captured `GraphqlHttpError` as
`ApiError::Graphql(value.to_string())`. Before this slice, the selected transport facade
returned that adapter error directly. `execute_selected_transport` then stored the string
inside the public `UiTransportError` envelope.

That display can contain:

- a GraphQL server error message;
- an HTTP status string;
- transport classification text.

Those values remain useful for private diagnostics but are not suitable as the final
Pricing storefront public contract.

## Boundary placement

`GraphqlCallContext` is created inside the selected GraphQL closure before
`graphql_adapter::fetch_storefront_pricing` is called. It receives the final adapter
`ApiError` before `execute_selected_transport` can construct a public `UiTransportError`.

Only `ApiError::Graphql` is reclassified. Existing `ApiError::ServerFn` query-validation
results pass through unchanged, so this slice does not change the Pricing validation
contract.

Each GraphQL fetch receives a unique correlation id in the namespace:

```text
pricing-storefront-graphql:fetch_storefront_pricing:<uuid>
```

## Public policy

| GraphQL transport condition | Public adapter message |
| --- | --- |
| Network failure | `Storefront pricing is temporarily unavailable` |
| Non-success HTTP response | `Storefront pricing is temporarily unavailable` |
| Unauthorized response | `Pricing storefront authentication is required` |
| GraphQL response rejection | `Pricing storefront request could not be completed` |
| Unrecognized captured display | `Pricing storefront request could not be completed` |

The selected path remains GraphQL, so `execute_selected_transport` preserves the outer
`UiTransportPath::Graphql` evidence. No fallback is introduced.

## Internal diagnostics

The original captured GraphQL display and parsed `GraphqlHttpError` remain private tracing
fields. The event also records:

- owner and owner operation;
- unique correlation id;
- whether a tenant slug is configured and its character length;
- selected handle presence and character length;
- locale presence and character length;
- currency-code presence and character length;
- region-id presence and character length;
- price-list-id presence and character length;
- channel-id presence and character length;
- channel-slug presence and character length;
- whether quantity was supplied;
- error kind;
- stable internal code;
- boundary name.

The structured fields do not contain tenant slug, selected handle, locale, currency code,
region id, price-list id, channel id, channel slug, or quantity values.

## Tracing dependency

The Pricing storefront package previously activated `tracing` only through the `ssr`
feature. The GraphQL-selected default profile also compiles the new diagnostics policy, so
`tracing` is now a normal workspace dependency and is no longer listed as `dep:tracing` in
the SSR feature.

This dependency-shape change does not alter native transport behavior. The native focused
guard is updated only to require the all-profile dependency form.

## Preserved behavior

This slice does not change:

- `StorefrontPricingQuery` fields or normalization;
- currency, UUID, resolution-context, or quantity validation messages;
- Pricing GraphQL query documents;
- GraphQL variables or tenant-header construction;
- list, detail, selected-handle, price-list, channel, or effective-price composition;
- Pricing native server-function endpoint;
- native runtime/context/owner error policy;
- native versus GraphQL selected transport;
- fallback behavior;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

Focused source evidence:

- `crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source-review.json`;
- `scripts/verify/verify-pricing-storefront-graphql-error-safety.mjs`.

The focused verifier is imported by:

- `scripts/verify/verify-pricing-storefront-boundary.mjs`.

All execution and runtime validation flags remain `false`. Source review alone does not
prove default, hydrate, SSR, browser, GraphQL runtime, mounted parity, workflow, CI, or
production behavior.

## Remaining work

The master ecommerce correlation-safe mapper cleanup remains open for:

- other remaining ecommerce storefront/admin adapters;
- inventory, customer, tax, promotion, and other non-`PortError` envelopes;
- runtime and mounted-parity evidence for Pricing storefront native and GraphQL paths.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-pricing-storefront-graphql-error-safety.mjs
node scripts/verify/verify-pricing-storefront-native-error-safety.mjs
node scripts/verify/verify-pricing-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing-storefront
cargo check -p rustok-pricing-storefront --features hydrate
cargo check -p rustok-pricing-storefront --features ssr
```
