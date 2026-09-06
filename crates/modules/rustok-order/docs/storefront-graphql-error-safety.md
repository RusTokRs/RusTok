# Order storefront GraphQL error safety

Status: source-unvalidated

## Scope

This source slice hardens both the public and private diagnostic boundary of the
order-owned storefront GraphQL transport for the single public checkout-completion
operation:

- `complete_storefront_checkout`.

The storefront facade remains the public transport consumer. The low-level GraphQL
adapter remains private and continues to own the mutation document, variables,
response decoding, cart UUID validation, checkout idempotency-key validation, and
DTO mapping.

## Rechecked gap

The public consumer boundary already reparsed the private adapter display handoff
through `GraphqlHttpError` and returned static order-owned messages. Server-provided
GraphQL messages and HTTP details therefore did not reach the storefront caller.

The structured event still copied two complete private values:

- `raw_error = %raw_error`;
- `parsed_error = ?parsed_error`.

The display text and Debug representation were unnecessary for correlation or the
closed five-category transport policy. This was a remaining non-`PortError`
diagnostic-envelope gap in the ecommerce correlation-safe mapper cleanup.

## Public consumer policy

`transport.rs` creates a private `GraphqlCallContext` before invoking the GraphQL
adapter. The context reparses the adapter display handoff through
`GraphqlHttpError::from_str` and maps only the `Graphql` transport variant.

The stable public messages remain:

| Typed failure | Public message |
| --- | --- |
| network | `Checkout completion is temporarily unavailable` |
| HTTP | `Checkout completion is temporarily unavailable` |
| unauthorized | `Checkout authentication is required` |
| GraphQL rejection | `Checkout request could not be completed` |
| unknown display envelope | `Checkout request could not be completed` |

`Validation` and `ServerFn` variants are returned unchanged. This preserves the
existing validation order and the independent native transport policy.

## Correlation and bounded diagnostics

Every selected GraphQL call creates a unique correlation identifier using the owner
operation and a new UUID. Internal tracing events retain only:

- owner `rustok_order.storefront`;
- owner operation `complete_storefront_checkout`;
- boundary `order_storefront_graphql_transport`;
- correlation identifier;
- one closed category: `network`, `http`, `unauthorized`, `graphql`, or `unknown`;
- stable internal code;
- raw-display presence and character length;
- whether typed `GraphqlHttpError` parsing succeeded;
- whether a tenant slug is configured and its character length;
- cart-id character length;
- idempotency-key character length;
- the `create_fulfillment` policy flag.

Raw GraphQL display text is not written to the event.
Debug output from the parsed typed error is not written to the event.

The mapper also does not log the raw tenant slug, cart id, idempotency key, command
metadata, GraphQL endpoint, mutation, variables, tokens, or returned checkout
objects.

Technical network, HTTP, and unknown failures continue to use error-level
diagnostics. Unauthorized and ordinary GraphQL rejection continue to use
warning-level diagnostics.

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
- Commerce delegation to the order-owned storefront facade;
- static public messages, stable codes, category severity, or non-GraphQL
  pass-through behavior.

The adapter still creates the exact typed GraphQL display handoff. Sanitization and
bounded diagnostics occur once at the public consumer boundary, preserving adapter
ownership.

## Source guard

The focused verifier is:

```bash
node scripts/verify/verify-order-storefront-graphql-error-safety.mjs
```

It checks typed mapping, static public messages and stable codes, correlation and safe
request-shape diagnostics, bounded raw-display presence/length and typed-parse
validity, absence of complete raw/debug payloads, GraphQL-only remapping,
private-adapter preservation, native-path preservation, truthful evidence status,
and validation nonclaims.

The general owner boundary verifier imports the focused guard:

```bash
node scripts/verify/verify-order-storefront-boundary.mjs
```

## Evidence boundary

Retained source evidence:

- `crates/rustok-order/contracts/evidence/storefront-graphql-error-safety-source.json`;
- `crates/rustok-order/contracts/evidence/storefront-graphql-error-safety-source-review.json`.

The evidence establishes only that the source and guardrail are present and reviewed.
It does not establish compilation, browser execution, GraphQL runtime behavior,
mounted Commerce parity, workflow success, CI success, or production behavior.

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

Source readiness must remain separate from runtime or promotion claims until those
commands and the required mounted failure evidence are retained on the same revision.
