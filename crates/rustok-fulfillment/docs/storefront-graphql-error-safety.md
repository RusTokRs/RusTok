# Fulfillment storefront GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the public GraphQL error boundary for the fulfillment-owned storefront shipping-selection operation:

- `select_storefront_shipping_option`;
- public facade `rustok_fulfillment_storefront::transport::select_shipping_option`;
- selected GraphQL transport path;
- boundary `fulfillment_storefront_graphql_transport`.

The private GraphQL adapter, native server functions, shipping-selection DTOs, selection planning, GraphQL document, feature-based transport selection, and Commerce compatibility delegation remain unchanged.

## Problem

The private GraphQL adapter intentionally converted `GraphqlHttpError` into the transport-owned variant:

```text
ShippingSelectionTransportError::Graphql(error.to_string())
```

Before this slice, the public transport facade delegated that variant directly to `execute_selected_transport`. `UiTransportError::graphql` stores the delegated display string in its public `graphql_error` field, so resolver messages and HTTP detail could reach the storefront caller.

This is a non-`PortError` public-envelope gap in the ecommerce correlation-safe mapper cleanup.

## Boundary policy

The public facade now creates a `GraphqlCallContext` before invoking the private adapter. Only `ShippingSelectionTransportError::Graphql` is classified. Other variants are returned unchanged.

The adapter display handoff is reparsed through the typed `GraphqlHttpError` contract:

| Internal category | Internal code | Public message | Severity |
| --- | --- | --- | --- |
| network | `fulfillment.storefront_graphql_network_unavailable` | `Shipping selection is temporarily unavailable` | error |
| non-success HTTP | `fulfillment.storefront_graphql_http_unavailable` | `Shipping selection is temporarily unavailable` | error |
| unauthorized | `fulfillment.storefront_graphql_authentication_required` | `Shipping selection authentication is required` | warning |
| GraphQL rejection | `fulfillment.storefront_graphql_request_rejected` | `Shipping selection request could not be completed` | warning |
| unknown display handoff | `fulfillment.storefront_graphql_unknown_failure` | `Shipping selection request could not be completed` | error |

The raw GraphQL detail is never copied into the returned `ShippingSelectionTransportError::Graphql`.

## Correlation and diagnostics

Each selected GraphQL invocation receives a generated correlation id with the fulfillment owner operation embedded in its prefix.

Internal events retain:

- owner `rustok_fulfillment.storefront`;
- owner operation `select_storefront_shipping_option`;
- boundary `fulfillment_storefront_graphql_transport`;
- per-call correlation id;
- parsed GraphQL error category;
- stable internal code;
- raw GraphQL display detail for operator diagnostics;
- whether a tenant slug is configured and its character length;
- cart-id character length;
- delivery-group count;
- shipping-profile-slug character length;
- seller-id presence;
- shipping-option-id presence.

The events deliberately do not retain:

- tenant slug value;
- cart id value;
- shipping profile slug value;
- seller id value;
- shipping option id value;
- available option ids;
- GraphQL endpoint;
- query document or variables;
- authorization token;
- Commerce command metadata.

## Public and transport stability

This work does not change:

- `ShippingSelectionTransportError` variants;
- `UiTransportError` structure;
- `SelectShippingOptionRequest` or delivery-group DTOs;
- request normalization;
- shipping-selection validation messages;
- shipping-selection planning or update construction;
- `SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION`;
- GraphQL variables or response mapping;
- configured tenant-slug header selection;
- native server-function endpoint or native safety policy;
- feature-based `NativeServer` versus `Graphql` selection;
- the no-fallback transport contract;
- Commerce delegation to the fulfillment-owned facade.

Validation errors remain pass-through because they are local, user-actionable failures and are not GraphQL runtime envelopes.

## Static evidence

`scripts/verify/verify-fulfillment-storefront-graphql-error-safety.mjs` guards:

- private adapter, safety, and native module separation;
- pre-call context construction and post-call GraphQL-only mapping;
- typed `GraphqlHttpError` classification;
- static public messages for every typed category;
- error versus warning severity;
- correlation and safe request-shape diagnostics;
- absence of raw request identity fields in diagnostics;
- unchanged private GraphQL adapter handoff and GraphQL document;
- unchanged native path and transport variant;
- unchanged explicit transport selection;
- unvalidated evidence flags.

The focused guard is imported by `verify-fulfillment-storefront-boundary.mjs` so the ordinary fulfillment storefront boundary check also rejects restoration of the raw public GraphQL mapping.

Evidence files:

- `crates/rustok-fulfillment/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-fulfillment/contracts/evidence/storefront-graphql-error-safety-source-review.json`.

## Evidence boundary

The source status is `fulfillment_storefront_graphql_error_safety_source_unvalidated`.

Source inspection does not prove:

- compilation;
- browser execution;
- real network, HTTP, unauthorized, or GraphQL failure injection;
- mounted Commerce parity;
- native/GraphQL behavior parity;
- workflow or CI success;
- FFA or FBA promotion;
- production readiness.

The broad ecommerce correlation-safe mapper task remains open for remaining storefront/admin GraphQL adapters, customer and other non-`PortError` envelopes, and runtime evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-storefront-graphql-error-safety.mjs
node scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs
node scripts/verify/verify-fulfillment-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-fulfillment-storefront
cargo check -p rustok-fulfillment-storefront --features hydrate
cargo check -p rustok-fulfillment-storefront --features ssr
```
