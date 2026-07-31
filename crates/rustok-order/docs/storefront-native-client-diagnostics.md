# Order storefront native client diagnostics

Status: **source-ready / unvalidated**

## Scope

This slice hardens the final native compatibility wrapper for the Order-owned storefront checkout completion operation:

- `complete_checkout`;
- `complete_checkout_server`;
- the private `NativeClientDiagnosticContext`.

The public native envelope was already static. This change replaces its uncorrelated outer log with a per-call, request-shape-only diagnostic boundary.

## Delivered source contract

`complete_checkout_server` creates `NativeClientDiagnosticContext` before moving the unchanged request into the generated server-function call. When that call fails, the wrapper records:

- owner and exact owner operation;
- per-call UUID correlation id;
- stable internal code and client transport boundary;
- cart-id and idempotency-key character lengths;
- command metadata string-field lengths;
- command-metadata presence;
- the original generated server-function error as a private diagnostic.

It does not log cart ids, idempotency keys, command metadata values, request payloads, checkout projections, tenant values, tokens, or provider details.

## Public error policy

The compatibility result remains:

```text
Checkout transport is temporarily unavailable
```

and remains represented as `CheckoutCompletionTransportError::ServerFn`. No new public error class, fallback, or message is introduced.

## Preserved behavior

This slice does not change:

- `crates/rustok-order/storefront/src/transport.rs`;
- explicit native/GraphQL transport selection or the no-fallback policy;
- the GraphQL adapter or `GraphqlCallContext` policy;
- `CompleteCheckoutRequest`, `CheckoutCompletion`, or adjustment DTOs;
- `order/complete-checkout` endpoint identity;
- mounted runtime dependency composition and context extraction;
- cart-id and idempotency-key validation;
- the Commerce staged checkout call or `StorefrontCheckoutCompletionCommand` payload;
- the SSR runtime error mapper and its public code/message diagnostics;
- Order FFA or FBA status.

## Static evidence

Source evidence is recorded in:

- `crates/rustok-order/contracts/evidence/storefront-native-client-diagnostics-source.json`;
- `crates/rustok-order/contracts/evidence/storefront-native-client-diagnostics-source-review.json`.

The focused guard is:

- `scripts/verify/verify-order-storefront-native-client-diagnostics.mjs`.

## Remaining gaps

Compilation, browser/hydrate behavior, mounted SSR behavior, remote transport parity, checkout execution and compensation evidence, and the wider ecommerce correlation-safe mapper cleanup remain open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-storefront-native-client-diagnostics.mjs
node scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs
node scripts/verify/verify-order-storefront-graphql-error-safety.mjs
node scripts/verify/verify-order-storefront-boundary.mjs
cargo check -p rustok-order-storefront --all-features
```
