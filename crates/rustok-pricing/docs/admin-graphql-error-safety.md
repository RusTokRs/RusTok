# Pricing admin GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the four Pricing admin GraphQL read operations selected by
`crates/rustok-pricing/admin/src/transport.rs`:

- `fetch_bootstrap`;
- `fetch_active_price_lists`;
- `fetch_products`;
- `fetch_product`.

The GraphQL documents, variables, request normalization, result mapping, native server
functions, and native-only Pricing mutations remain unchanged.

## Confirmed gap

The Pricing admin GraphQL adapter captures `GraphqlHttpError` as
`ApiError::Graphql(value.to_string())`. Before this slice, the selected transport facade
returned that display directly. GraphQL server messages, HTTP status text, and transport
classification details could therefore cross the final admin UI boundary.

## Boundary placement

Each GraphQL branch now creates a `GraphqlCallContext` before calling the adapter. The
context receives the final `ApiError` after the adapter returns and before the admin UI can
render it.

Only `ApiError::Graphql` is reclassified. Existing `ApiError::ServerFn` request-validation
results pass through unchanged.

Every call receives a unique correlation id in the namespace:

```text
pricing-admin-graphql:<operation>:<uuid>
```

## Public policy

| GraphQL condition | Public message |
| --- | --- |
| Network failure | `Pricing admin service is temporarily unavailable` |
| Non-success HTTP response | `Pricing admin service is temporarily unavailable` |
| Unauthorized response | `Pricing admin authentication is required` |
| GraphQL response rejection | `Pricing admin request could not be completed` |
| Unrecognized captured display | `Pricing admin request could not be completed` |

No fallback is introduced, and the selected transport path remains unchanged.

## Internal diagnostics

The original captured GraphQL display and parsed `GraphqlHttpError` remain private tracing
fields. Structured diagnostics record only:

- owner, operation, boundary, stable code, error kind, and correlation id;
- tenant slug presence and character length;
- tenant id and requested resource id presence and character length;
- locale, search, status, currency, region, price-list, channel id, and channel slug
  presence and character length;
- quantity presence.

The actual tenant, identifier, locale, search, status, pricing-context, channel, or
quantity values are not recorded as structured fields.

## Preserved behavior

This slice does not change:

- Pricing admin GraphQL queries or variables;
- bootstrap, active-price-list, product-list, or product-detail response mapping;
- UUID, channel, resolution-context, locale, search, or status normalization;
- native server-function reads;
- variant price, discount, price-list rule, or price-list scope mutations;
- native versus GraphQL selection;
- fallback behavior;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

- `crates/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source.json`;
- `crates/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source-review.json`;
- `scripts/verify/verify-pricing-admin-graphql-error-safety.mjs`.

The focused verifier is imported by `scripts/verify/verify-pricing-admin-boundary.mjs`.
All execution flags remain `false`; source review alone does not prove compilation,
GraphQL/browser runtime, or mounted parity.

## Remaining work

The ecommerce correlation-safe mapper cleanup remains open for other storefront/admin
adapters and non-`PortError` public envelopes, including inventory, customer, tax,
promotion, and remaining owner-specific paths.

## Suggested maintainer checks

```bash
node scripts/verify/verify-pricing-admin-graphql-error-safety.mjs
node scripts/verify/verify-pricing-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing-admin
cargo check -p rustok-pricing-admin --features hydrate
cargo check -p rustok-pricing-admin --features ssr
```
