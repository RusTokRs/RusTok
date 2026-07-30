# Order storefront checkout runtime error diagnostics

Status: **source-ready / unvalidated**

## Scope

This slice hardens the runtime-error mapper used by the Order-owned native checkout endpoint:

- `crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs`.

The underlying Commerce runtime already exposes stable public checkout codes and messages. This slice keeps that public contract intact and adds correlation-safe SSR diagnostics at the Order transport boundary.

## Delivered source contract

When `complete_storefront_checkout_with_product_port` returns `StorefrontStagedCheckoutRuntimeError`, the native adapter logs:

- Order storefront owner;
- exact owner operation;
- request correlation id;
- tenant id;
- channel id and channel slug;
- locale;
- stable internal code and transport boundary;
- public checkout code and retryability;
- the original runtime error on the server.

The returned `ServerFnError` remains exactly:

```text
<public_code>: <public_message>
```

Both values still come from `StorefrontStagedCheckoutRuntimeError::public_code()` and `public_message()`.

## Preserved behavior

This slice does not change:

- the `order/complete-checkout` endpoint;
- `CompleteCheckoutRequest` or `CheckoutCompletion` DTOs;
- event-bus, payment-provider, or Product runtime composition;
- request, tenant, or optional-auth extraction;
- cart-id or idempotency-key validation messages;
- `StorefrontCheckoutCompletionCommand` fields or metadata payload;
- checkout completion DTO mapping;
- the outer `CheckoutCompletionTransportError::ServerFn("Checkout transport is temporarily unavailable")` envelope;
- GraphQL transport or transport selection.

## Static evidence

`scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs` guards the structured diagnostics, unchanged public envelope, endpoint and command payload, validation messages, dependency composition, and source-only validation flags.

## Remaining gaps

Compilation, mounted parity, native runtime, remote transport, browser evidence, compensation execution evidence, and the broader ecommerce mapper cleanup remain open.

## Suggested maintainer checks

```bash
node scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs
node scripts/verify/verify-order-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order-storefront --all-features
```

These commands were intentionally not run by the implementation agent.
