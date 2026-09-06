# Payment storefront GraphQL error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens both the public and private diagnostic boundary for the
payment-owned storefront GraphQL transport used when `UiTransportPath::Graphql`
is selected for:

- `create_payment_collection`;
- `fetch_payment_collection`;
- `fetch_refund_summary`.

The private low-level adapter, GraphQL documents, variables, DTO mapping, native
server functions, and explicit transport selection remain unchanged.

## Rechecked gap

The public transport consumer already reparsed each private adapter display
handoff through `GraphqlHttpError::from_str` and returned static payment-owned
messages. GraphQL server messages and HTTP/client detail therefore did not cross
the public storefront envelope.

The shared structured event still copied two complete private values:

- `raw_error = %raw_error`;
- `parsed_error = ?parsed_error`.

Those payloads were unnecessary for correlation, exact owner-operation
attribution, or the closed five-category transport policy. This was a remaining
non-`PortError` diagnostic-envelope gap in the ecommerce correlation-safe mapper
cleanup.

## Consumer safety boundary

The low-level adapter remains private and unchanged. The public payment transport
consumer creates a `GraphqlCallContext` immediately before each GraphQL call and
maps only a returned `PaymentTransportError::Graphql`.

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

The original raw display is not copied into the returned
`PaymentTransportError`. Validation and native variants remain pass-through.

## Correlation and bounded diagnostics

Every selected GraphQL operation receives a unique correlation id with the
shape:

```text
payment-storefront-graphql:{owner_operation}:{uuid}
```

Internal events retain only:

- truthful owner `rustok_payment.storefront`;
- exact owner operation;
- correlation id;
- tenant slug configured/not-configured fact;
- trimmed tenant slug character length when configured;
- one closed error category: `network`, `http`, `unauthorized`, `graphql`, or
  `unknown`;
- stable internal code;
- raw-display presence and character length;
- whether typed `GraphqlHttpError` parsing succeeded;
- boundary `payment_storefront_graphql_transport`.

Raw GraphQL display text is not written to the event.
Debug output from the parsed typed error is not written to the event.

Diagnostics also exclude:

- the tenant slug value;
- authentication tokens or authorization headers;
- endpoint URLs;
- GraphQL query text;
- GraphQL variables;
- cart or order request identifiers;
- command metadata.

## Covered operations

The public facade retains the following exact owner operations:

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
- no-fallback behavior;
- static public messages, stable codes, category severity, or non-GraphQL
  pass-through behavior.

## Static evidence

`scripts/verify/verify-payment-storefront-graphql-error-safety.mjs` guards:

- private adapter and safety-module visibility;
- all three consumer mappings and exact owner operations;
- typed display parsing and all closed error variants;
- stable codes and public messages;
- correlation and tenant-shape diagnostics;
- bounded raw-display presence/length and typed-parse validity;
- absence of raw GraphQL display text and parsed-error Debug output;
- absence of raw tenant, query, variable, token, endpoint, request-id, and
  metadata fields;
- unchanged native calls and transport selection;
- unchanged low-level GraphQL handoffs;
- truthful source/review evidence and unvalidated flags.

## Remaining work

The ecommerce correlation-safe mapper item remains open for payment checkout
execution and compensation, remaining storefront/admin GraphQL adapters, other
non-`PortError` public and diagnostic envelopes, and runtime evidence. This slice
does not promote FFA, FBA, transport, browser, runtime, or production status.

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
