# Cart storefront GraphQL error safety

Status: source-unvalidated

## Scope

This source slice hardens the cart-owned storefront GraphQL transport for the
three public operations:

- `fetch_cart`;
- `decrement_line_item`;
- `remove_line_item`.

The storefront transport facade remains the public consumer. The low-level
GraphQL adapter remains private and continues to own the query and mutation
documents, variables, response decoding, cart and line-item UUID validation,
quantity-command behavior, and DTO mapping.

## Previous gap

The private adapter converted `GraphqlHttpError` into
`ApiError::Graphql(value.to_string())`. Each selected GraphQL closure delegated
directly to that adapter, and `execute_selected_transport` retained the display
string in the public `UiTransportError.graphql_error` field.

Consequently, server-provided GraphQL messages and HTTP details could cross the
cart storefront UI boundary without a cart-owned public-envelope policy. The
native SSR path already had a separate static-envelope policy, but that policy
did not cover the GraphQL transport.

## Public consumer policy

`transport/mod.rs` now creates a private `GraphqlCallContext` before invoking
each GraphQL operation. The context reparses the adapter's display handoff
through `GraphqlHttpError::from_str` and maps only `ApiError::Graphql`.

The stable public messages are:

| Typed failure | Public message |
| --- | --- |
| network | `Cart storefront is temporarily unavailable` |
| HTTP | `Cart storefront is temporarily unavailable` |
| unauthorized | `Cart authentication is required` |
| GraphQL rejection | `Cart request could not be completed` |
| unknown display envelope | `Cart request could not be completed` |

`Validation` and `ServerFn` variants are returned unchanged. This preserves the
existing identifier-validation behavior and independent native SSR policy.

## Correlation diagnostics

Every selected GraphQL call creates a unique correlation identifier from the
owner operation and a new UUID. Internal tracing events retain:

- owner `rustok_cart.storefront`;
- owner operation `fetch_cart`, `decrement_line_item`, or `remove_line_item`;
- boundary `cart_storefront_graphql_transport`;
- correlation identifier;
- typed error kind and stable code;
- raw and reparsed GraphQL cause for internal diagnosis;
- whether a tenant slug is configured and its character length;
- selected-cart and locale presence plus character lengths for reads;
- cart-id and line-item-id character lengths for writes;
- decrement command kind, without quantity or identifier values.

The mapper does not log the raw tenant slug, selected cart id, locale, cart id,
line-item id, GraphQL endpoint, documents, variables, tokens, response payloads,
or cart/customer data.

Technical network, HTTP, and unknown failures use error-level diagnostics.
Unauthorized and ordinary GraphQL rejection use warning-level diagnostics.

## Preserved behavior

This slice does not change:

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
- Commerce aggregate use of the cart storefront facade.

The adapter still creates the same typed GraphQL display handoff. Sanitization
occurs once at the public consumer boundary, avoiding duplicate diagnostics and
preserving adapter ownership.

## Source guard

The focused verifier is:

```bash
node scripts/verify/verify-cart-storefront-graphql-error-safety.mjs
```

It checks all three operation contexts, typed mapping policy, static public
messages and codes, correlation and safe request-shape diagnostics, GraphQL-only
remapping, private-adapter preservation, native-path preservation, evidence
status, and validation nonclaims.

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
