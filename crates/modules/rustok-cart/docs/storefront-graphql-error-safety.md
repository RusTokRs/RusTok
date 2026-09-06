# Cart storefront GraphQL error safety

Status: source-unvalidated

## Scope

This source slice hardens the public and diagnostic boundary for the three
cart-owned storefront GraphQL operations:

- `fetch_cart`;
- `decrement_line_item`;
- `remove_line_item`.

The storefront transport facade remains the public consumer. The low-level
GraphQL adapter remains private and continues to own the query and mutation
documents, variables, response decoding, cart and line-item UUID validation,
quantity-command behavior, and DTO mapping.

## Rechecked gap

The Cart storefront facade already creates an operation-specific
`GraphqlCallContext`, reparses the private adapter display through
`GraphqlHttpError::from_str`, and returns only stable Cart-owned public messages.
GraphQL server messages and HTTP details therefore do not cross the public
`UiTransportError` envelope.

The shared tracing event still copied two complete private values:

- `raw_error = %raw_error`;
- `parsed_error = ?parsed_error`.

Those payloads were not required for correlation, exact operation attribution,
the closed five-category policy, or request-shape diagnosis. They were a
remaining non-`PortError` diagnostic-envelope gap in the ecommerce
correlation-safe mapper cleanup.

## Public consumer policy

`transport/mod.rs` continues to create a private `GraphqlCallContext` before
invoking each GraphQL operation. The context maps only `ApiError::Graphql` after
the adapter returns and before the selected transport creates the public error
envelope.

The stable public messages remain:

| Typed failure | Public message |
| --- | --- |
| network | `Cart storefront is temporarily unavailable` |
| HTTP | `Cart storefront is temporarily unavailable` |
| unauthorized | `Cart authentication is required` |
| GraphQL rejection | `Cart request could not be completed` |
| unknown display envelope | `Cart request could not be completed` |

`Validation` and `ServerFn` variants are returned unchanged. This preserves the
existing identifier-validation behavior and independent native SSR policy.

Technical network, HTTP, and unknown failures retain error-level diagnostics.
Unauthorized and ordinary GraphQL rejection retain warning-level diagnostics.

## Bounded correlation diagnostics

Every selected GraphQL call retains a unique correlation identifier from the
owner operation and a new UUID. Structured events retain only:

- owner `rustok_cart.storefront`;
- owner operation `fetch_cart`, `decrement_line_item`, or `remove_line_item`;
- boundary `cart_storefront_graphql_transport`;
- correlation identifier;
- one closed error category: `network`, `http`, `unauthorized`, `graphql`, or
  `unknown`;
- stable internal code;
- raw-display presence and character length;
- whether typed `GraphqlHttpError` parsing succeeded;
- whether a tenant slug is configured and its character length;
- selected-cart and locale presence plus character lengths for reads;
- cart-id and line-item-id character lengths for writes;
- decrement command kind, without quantity or identifier values.

Raw GraphQL display text is not written to the event.
Debug output from the parsed typed error is not written to the event.

The mapper also does not log the raw tenant slug, selected cart id, locale, cart
id, line-item id, GraphQL endpoint, documents, variables, tokens, response
payloads, or cart/customer data.

## Preserved behavior

This work does not change:

- feature-based `NativeServer` versus `Graphql` selection;
- the no-fallback transport policy;
- the private GraphQL adapter;
- `STOREFRONT_CART_QUERY`;
- `UPDATE_STOREFRONT_CART_LINE_ITEM_MUTATION`;
- `REMOVE_STOREFRONT_CART_LINE_ITEM_MUTATION`;
- GraphQL variables or response DTOs;
- cart and line-item identifier validation;
- decrement quantity-command policy;
- native server functions or native SSR error mapping;
- stable public messages, codes, or severity;
- Commerce aggregate use of the cart storefront facade;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

The adapter still creates the same typed GraphQL display handoff. Sanitization
continues to occur once at the public consumer boundary, avoiding duplicate
diagnostics and preserving adapter ownership.

## Source guard

The focused verifier is:

```bash
node scripts/verify/verify-cart-storefront-graphql-error-safety.mjs
```

It now requires bounded error facts, forbids the complete raw and parsed payload
fields, checks all three operation contexts and static public envelopes, and
reconciles the retained source/review evidence and this documentation.

The general owner boundary verifier imports both native and GraphQL guards:

```bash
node scripts/verify/verify-cart-storefront-boundary.mjs
```

## Evidence boundary

Retained source evidence:

- `crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source-review.json`.

The evidence establishes only that the source and guardrail are present and
have been reviewed. It does not establish compilation, browser execution,
GraphQL runtime behavior, mounted Commerce parity, workflow success, CI success,
or production behavior.

The broad ecommerce correlation-safe mapper cleanup remains open.

No tests, verifiers, Cargo commands, formatting, workflows, or CI were run per
maintainer instruction.

## Suggested maintainer verification

```bash
node scripts/verify/verify-cart-storefront-graphql-error-safety.mjs
node scripts/verify/verify-cart-storefront-native-error-safety.mjs
node scripts/verify/verify-cart-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart-storefront
cargo check -p rustok-cart-storefront --features hydrate
cargo check -p rustok-cart-storefront --features ssr
```

Source readiness must remain separate from runtime or promotion claims until
those commands and the required mounted failure evidence are retained on the
same revision.
