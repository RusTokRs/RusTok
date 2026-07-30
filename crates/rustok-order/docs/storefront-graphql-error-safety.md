# Order storefront GraphQL error safety

Status: source-unvalidated

## Scope

This source slice hardens the order-owned storefront GraphQL transport for the
single public checkout-completion operation:

- `complete_storefront_checkout`.

The storefront facade remains the public transport consumer. The low-level
GraphQL adapter remains private and continues to own the mutation document,
variables, response decoding, cart UUID validation, checkout idempotency-key
validation, and DTO mapping.

## Previous gap

The selected GraphQL closure delegated directly to the private adapter. The
adapter converted `GraphqlHttpError` into
`CheckoutCompletionTransportError::Graphql(error.to_string())`, and
`execute_selected_transport` retained that display string in the public
`UiTransportError.graphql_error` field.

Consequently, server-provided GraphQL messages and HTTP status details could
cross the storefront UI boundary without an order-owned public-envelope policy.
The native server-function path already had a separate sanitized runtime policy,
but that policy did not cover the GraphQL path.

## Public consumer policy

`transport.rs` now creates a private `GraphqlCallContext` before invoking the
GraphQL adapter. The context reparses the adapter's display handoff through
`GraphqlHttpError::from_str` and maps only the `Graphql` transport variant.

The stable public messages are:

| Typed failure | Public message |
| --- | --- |
| network | `Checkout completion is temporarily unavailable` |
| HTTP | `Checkout completion is temporarily unavailable` |
| unauthorized | `Checkout authentication is required` |
| GraphQL rejection | `Checkout request could not be completed` |
| unknown display envelope | `Checkout request could not be completed` |

`Validation` and `ServerFn` variants are returned unchanged. This preserves the
existing validation order and the independent native transport policy.

## Correlation diagnostics

Every selected GraphQL call creates a unique correlation identifier using the
owner operation and a new UUID. Internal tracing events retain:

- owner `rustok_order.storefront`;
- owner operation `complete_storefront_checkout`;
- boundary `order_storefront_graphql_transport`;
- correlation identifier;
- typed error kind and stable code;
- raw and reparsed GraphQL cause for internal diagnosis;
- whether a tenant slug is configured and its character length;
- cart-id character length;
- idempotency-key character length;
- the `create_fulfillment` policy flag.

The mapper does not log the raw tenant slug, cart id, idempotency key, command
metadata, GraphQL endpoint, mutation, variables, tokens, or returned checkout
objects.

Technical network, HTTP, and unknown failures use error-level diagnostics.
Unauthorized and ordinary GraphQL rejection use warning-level diagnostics.

## Preserved behavior

This slice does not change:

- feature-based `NativeServer` versus `Graphql` selection;
- the no-fallback transport policy;
- the private GraphQL adapter;
- `COMPLETE_STOREFRONT_CHECKOUT_MUTATION`;
- GraphQL variables or response DTOs;
- cart UUID validation;
- the 1-to-191-byte checkout idempotency-key rule;
- checkout command metadata;
- native server functions or native runtime mapping;
- Commerce delegation to the order-owned storefront facade.

The adapter still creates the exact typed GraphQL display handoff. Sanitization
occurs once at the public consumer boundary, avoiding duplicate diagnostics and
preserving adapter ownership.

## Source guard

The focused verifier is:

```bash
node scripts/verify/verify-order-storefront-graphql-error-safety.mjs
```

It checks the typed mapping policy, static public messages, stable codes,
correlation and safe request-shape diagnostics, GraphQL-only remapping,
private-adapter preservation, native-path preservation, evidence status, and
validation nonclaims.

The general owner boundary verifier imports the focused guard:

```bash
node scripts/verify/verify-order-storefront-boundary.mjs
```

## Evidence boundary

Retained source evidence:

- `crates/rustok-order/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-order/contracts/evidence/storefront-graphql-error-safety-source-review.json`.

The evidence establishes only that the source and guardrail are present and
have been reviewed. It does not establish compilation, browser execution,
GraphQL runtime behavior, mounted Commerce parity, workflow success, CI success,
or production behavior.

The broad ecommerce correlation-safe mapper cleanup remains open.

No tests, verifiers, Cargo commands, formatting, workflows, or CI were run per
maintainer instruction.

## Suggested maintainer verification

```bash
node scripts/verify/verify-order-storefront-graphql-error-safety.mjs
node scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs
node scripts/verify/verify-order-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order-storefront
cargo check -p rustok-order-storefront --features hydrate
cargo check -p rustok-order-storefront --features ssr
```

Source readiness must remain separate from runtime or promotion claims until
those commands and the required mounted failure evidence are retained on the
same revision.
