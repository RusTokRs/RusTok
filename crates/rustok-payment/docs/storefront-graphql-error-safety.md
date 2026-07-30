# Payment storefront GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the payment-owned storefront GraphQL transport used when
`UiTransportPath::Graphql` is selected for:

- `create_payment_collection`;
- `fetch_payment_collection`;
- `fetch_refund_summary`.

The private low-level adapter, GraphQL documents, variables, DTO mapping, native
server functions, and explicit transport selection remain unchanged.

## Problem

`rustok_graphql::execute` returns a typed `GraphqlHttpError`, but the private
payment GraphQL adapter converted each failure to
`PaymentTransportError::Graphql(error.to_string())`. The public transport facade
then passed that display string into `UiTransportError.graphql_error`.

That made GraphQL server messages and HTTP/client details part of a public UI
transport envelope. Native storefront calls already used stable owner-owned
messages, so the two payment transport paths had different error-safety policy.

## Consumer safety boundary

The low-level adapter remains private and unchanged. The public payment
transport consumer now creates a `GraphqlCallContext` immediately before each
GraphQL call and maps only a returned `PaymentTransportError::Graphql`.

`PaymentTransportError::Validation` and `PaymentTransportError::ServerFn` are
returned unchanged. There is no native-to-GraphQL fallback and transport
selection remains explicit.

## Typed policy

The consumer reparses the private adapter display using
`GraphqlHttpError::from_str`, whose display round-trip is owned by
`rustok-graphql`.

| Internal variant | Stable code | Public message | Severity |
| --- | --- | --- | --- |
| `Network` | `payment.storefront_graphql_network_unavailable` | `Payment storefront is temporarily unavailable` | error |
| `Http(_)` | `payment.storefront_graphql_http_unavailable` | `Payment storefront is temporarily unavailable` | error |
| `Unauthorized` | `payment.storefront_graphql_authentication_required` | `Payment storefront authentication is required` | warning |
| `Graphql(_)` | `payment.storefront_graphql_request_rejected` | `Payment storefront request could not be completed` | warning |
| unknown display | `payment.storefront_graphql_unknown_failure` | `Payment storefront request could not be completed` | error |

The original raw display and parsed typed result remain internal diagnostic
fields. Neither is copied into the returned `PaymentTransportError`.

## Correlation and diagnostics

Every selected GraphQL operation receives a unique correlation id with the
shape:

```text
payment-storefront-graphql:{owner_operation}:{uuid}
```

Internal events include:

- truthful owner `rustok_payment.storefront`;
- exact owner operation;
- correlation id;
- tenant slug configured/not-configured fact;
- trimmed tenant slug character length when configured;
- typed error kind;
- stable code;
- boundary `payment_storefront_graphql_transport`;
- raw and parsed error details for server diagnostics only.

Diagnostics deliberately exclude:

- the tenant slug value;
- authentication tokens or authorization headers;
- endpoint URLs;
- GraphQL query text;
- GraphQL variables;
- cart or order request identifiers;
- command metadata.

## Covered operations

The public facade maps the following owner operations:

- `create_storefront_payment_collection`;
- `read_storefront_payment_collection`;
- `read_storefront_order_refunds`.

The private adapter still executes the same named queries and mutation and
preserves existing serialization and response mapping.

## Preserved behavior

This work does not change:

- `PaymentTransportError` variants;
- `UiTransportError` shape;
- payment request or response DTOs;
- GraphQL documents or variables;
- configured tenant-slug lookup order;
- decimal refund aggregation;
- payment collection field mapping;
- native server-function behavior;
- feature-based native/GraphQL transport selection;
- no-fallback behavior.

## Static evidence

`scripts/verify/verify-payment-storefront-graphql-error-safety.mjs` guards:

- private adapter and safety-module visibility;
- all three consumer mappings;
- exact owner operations;
- typed display parsing and all error variants;
- stable codes and public messages;
- correlation and tenant-shape diagnostics;
- absence of raw tenant, query, variable, token, endpoint, request-id, and
  metadata fields;
- unchanged native calls and transport selection;
- unchanged low-level GraphQL handoffs;
- unvalidated evidence flags.

## Remaining work

The ecommerce correlation-safe mapper item remains open for other payment,
fulfillment, order, customer, tax, promotion, ecommerce, and non-`PortError`
public envelopes. This slice does not promote FFA, FBA, transport, browser,
runtime, or production status.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-storefront-graphql-error-safety.mjs
node scripts/verify/verify-payment-storefront-native-error-safety.mjs
node scripts/verify/verify-payment-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment-storefront
cargo check -p rustok-payment-storefront --features hydrate
cargo check -p rustok-payment-storefront --features ssr
```
