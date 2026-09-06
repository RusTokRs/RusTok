# Pricing storefront GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Pricing storefront GraphQL public and diagnostic error
boundary for the final GraphQL branch of `fetch_storefront_pricing`.

The GraphQL adapter query documents, variables, list/detail composition, result
mapping, Pricing query validation, native server-function adapter, transport
selection, and all-profile tracing dependency shape remain unchanged.

## Rechecked gap

The selected Pricing GraphQL facade already creates `GraphqlCallContext`, reparses
the private adapter display through `GraphqlHttpError::from_str`, and returns only
stable Pricing-owned public messages. GraphQL server messages and HTTP detail
therefore do not cross the public `UiTransportError` envelope.

The shared structured event still copied two complete private values:

- `raw_error = %raw_error`;
- `parsed_error = ?parsed_error`.

Those payloads were not required for correlation, the closed five-category policy,
or query-shape diagnosis. They were a remaining non-`PortError` diagnostic-envelope
gap in the ecommerce correlation-safe mapper cleanup.

## Boundary placement

`GraphqlCallContext` remains inside the selected GraphQL closure and is created
before `graphql_adapter::fetch_storefront_pricing` is called. It maps only
`ApiError::Graphql` before `execute_selected_transport` constructs the public
transport envelope.

Existing `ApiError::ServerFn` query-validation results pass through unchanged.
No native-to-GraphQL fallback is introduced.

Each GraphQL fetch retains a unique correlation id in the namespace:

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

Technical network, HTTP, and unknown failures retain error severity. Unauthorized
and ordinary GraphQL rejection retain warning severity.

## Bounded internal diagnostics

The event retains only:

- owner and owner operation;
- unique correlation id;
- whether a tenant slug is configured and its character length;
- selected-handle, locale, currency-code, region-id, price-list-id, channel-id,
  and channel-slug presence and character lengths;
- whether quantity was supplied;
- one closed category: `network`, `http`, `unauthorized`, `graphql`, or `unknown`;
- stable internal code;
- raw-display presence and character length;
- whether typed `GraphqlHttpError` parsing succeeded;
- boundary name.

Raw GraphQL display text is not written to the event.
Debug output from the parsed typed error is not written to the event.

The structured fields also do not contain tenant slug, selected handle, locale,
currency code, region id, price-list id, channel id, channel slug, or quantity
values.

## Tracing dependency

The earlier Pricing storefront safety slice made `tracing` a normal workspace
dependency because the GraphQL-selected default profile compiles the same policy.
This diagnostic-only follow-up does not change `Cargo.toml`, SSR features, or native
transport behavior.

## Preserved behavior

This work does not change:

- `StorefrontPricingQuery` fields or normalization;
- currency, UUID, resolution-context, or quantity validation messages;
- Pricing GraphQL query documents, variables, or tenant-header construction;
- list, detail, selected-handle, price-list, channel, or effective-price
  composition;
- Pricing native server-function endpoint and public policy;
- native versus GraphQL selected transport;
- public error variants, stable messages, stable codes, or severity;
- fallback behavior;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

Focused source evidence:

- `crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source-review.json`;
- `scripts/verify/verify-pricing-storefront-graphql-error-safety.mjs`.

The focused verifier now requires bounded error facts, forbids the complete raw and
parsed payload fields, preserves the Pricing query and transport contracts, and
checks truthful source/review evidence.

All execution and runtime validation flags remain `false`. Source review alone does
not prove default, hydrate, SSR, browser, GraphQL runtime, mounted parity, workflow,
CI, or production behavior.

## Remaining work

The master ecommerce correlation-safe mapper cleanup remains open for other
storefront/admin adapters, payment and fulfillment execution diagnostics, remaining
non-`PortError` public or diagnostic envelopes, and runtime or mounted-parity
evidence.

No tests, verifiers, Cargo commands, formatting, workflows, or CI were run per
maintainer instruction.

## Suggested maintainer checks

```bash
node scripts/verify/verify-pricing-storefront-graphql-error-safety.mjs
node scripts/verify/verify-pricing-storefront-native-error-safety.mjs
node scripts/verify/verify-pricing-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing-storefront
cargo check -p rustok-pricing-storefront --features hydrate
cargo check -p rustok-pricing-storefront --features ssr
```
