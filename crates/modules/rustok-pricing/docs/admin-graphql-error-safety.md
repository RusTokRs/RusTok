# Pricing admin GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the public and diagnostic boundary for the four Pricing admin
GraphQL read operations selected by
`crates/rustok-pricing/admin/src/transport.rs`:

- `fetch_bootstrap`;
- `fetch_active_price_lists`;
- `fetch_products`;
- `fetch_product`.

The GraphQL documents, variables, request normalization, result mapping, native
server functions, and native-only Pricing mutations remain unchanged.

## Rechecked gap

The Pricing admin transport already creates a `GraphqlCallContext` before each
adapter call, reparses the private adapter display through
`GraphqlHttpError::from_str`, and returns only stable Pricing-owned messages.
GraphQL server messages and HTTP status text therefore do not cross the final
admin UI envelope.

The shared tracing event still copied two complete private values:

- `raw_error = %raw_error`;
- `parsed_error = ?parsed_error`.

Those payloads were not required for correlation, exact operation attribution,
the closed five-category policy, or request-shape diagnosis. They were a
remaining non-`PortError` diagnostic-envelope gap in the ecommerce
correlation-safe mapper cleanup.

## Boundary placement

Each GraphQL branch retains a `GraphqlCallContext` created before the private
adapter call. The context maps only `ApiError::Graphql` after the adapter returns
and before the admin UI can render the selected transport error.

Existing `ApiError::ServerFn` request-validation results pass through unchanged.
Every call retains a unique correlation id in the namespace:

```text
pricing-admin-graphql:<operation>:<uuid>
```

No fallback is introduced, and the selected transport path remains unchanged.

## Public policy

| GraphQL condition | Public message |
| --- | --- |
| Network failure | `Pricing admin service is temporarily unavailable` |
| Non-success HTTP response | `Pricing admin service is temporarily unavailable` |
| Unauthorized response | `Pricing admin authentication is required` |
| GraphQL response rejection | `Pricing admin request could not be completed` |
| Unrecognized captured display | `Pricing admin request could not be completed` |

Technical network, HTTP, and unknown failures retain error severity.
Unauthorized and ordinary GraphQL rejection retain warning severity.

## Bounded internal diagnostics

Structured events retain only:

- owner, exact operation, boundary, stable code, closed error category, and
  correlation id;
- tenant slug presence and character length;
- tenant id and requested resource id presence and character length;
- locale, search, status, currency, region, price-list, channel id, and channel
  slug presence and character length;
- quantity presence;
- raw-display presence and character length;
- whether typed `GraphqlHttpError` parsing succeeded.

Raw GraphQL display text is not written to the event.
Debug output from the parsed typed error is not written to the event.

The actual tenant, identifier, locale, search, status, pricing-context, channel,
or quantity values are also not recorded as structured fields.

## Preserved behavior

This work does not change:

- Pricing admin GraphQL queries or variables;
- bootstrap, active-price-list, product-list, or product-detail response mapping;
- UUID, channel, resolution-context, locale, search, or status normalization;
- request-validation messages or non-GraphQL pass-through;
- native server-function reads;
- variant price, discount, price-list rule, or price-list scope mutations;
- native versus GraphQL selection;
- stable public messages, codes, category severity, or fallback behavior;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

- `crates/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source.json`;
- `crates/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source-review.json`;
- `scripts/verify/verify-pricing-admin-graphql-error-safety.mjs`.

The focused verifier is imported by
`scripts/verify/verify-pricing-admin-boundary.mjs`. It requires bounded error
facts, forbids complete raw and parsed payload fields, preserves all four read
operations and native-only mutations, and checks truthful source/review evidence.

All execution flags remain `false`; source review alone does not prove
compilation, GraphQL/browser runtime, mounted parity, workflow, CI, or production
behavior.

## Remaining work

The ecommerce correlation-safe mapper cleanup remains open for other
storefront/admin adapters, payment and fulfillment execution diagnostics,
remaining non-`PortError` public or diagnostic envelopes, and runtime or
mounted-parity evidence.

No tests, verifiers, Cargo commands, formatting, workflows, or CI were run per
maintainer instruction.

## Suggested maintainer checks

```bash
node scripts/verify/verify-pricing-admin-graphql-error-safety.mjs
node scripts/verify/verify-pricing-admin-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing-admin
cargo check -p rustok-pricing-admin --features hydrate
cargo check -p rustok-pricing-admin --features ssr
```
