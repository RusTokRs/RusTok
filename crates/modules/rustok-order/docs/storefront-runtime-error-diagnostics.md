# Order storefront checkout runtime error diagnostics

Status: **source-ready / unvalidated**

## Scope

This slice hardens the context-extraction and runtime-error mappers used by the Order-owned native checkout endpoint:

- `crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs`.

The underlying Commerce runtime already exposes stable public checkout codes and messages. This slice keeps that public contract intact while limiting Order transport diagnostics to bounded type and request-shape facts.

## Delivered source contract

Context extraction failures record only:

- the Rust framework error type;
- Order storefront owner and the exact extraction operation;
- a stable internal code and native boundary.

In all diagnostic branches, complete context and runtime errors are not logged.

When `complete_storefront_checkout_with_product_port` returns `StorefrontStagedCheckoutRuntimeError`, the native adapter records:

- the concrete runtime error type;
- Order storefront owner and exact owner operation;
- the server-generated correlation id;
- whether the tenant UUID is non-nil;
- channel UUID presence and optional non-nil state;
- channel-slug presence and optional character length;
- locale presence and character length;
- the stable public checkout code and retryability;
- the stable internal code and native boundary.

Raw tenant and request-context identity values are not logged. Channel IDs, channel slugs, locale text, idempotency keys, and complete framework or owner errors are absent from the mounted diagnostic events.

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
- request, tenant, or optional-auth extraction order;
- cart-id or idempotency-key validation messages;
- `StorefrontCheckoutCompletionCommand` fields or metadata payload;
- the Commerce checkout owner call;
- checkout completion DTO mapping;
- the outer `CheckoutCompletionTransportError::ServerFn("Checkout transport is temporarily unavailable")` envelope;
- GraphQL transport or transport selection;
- Order FFA or FBA status.

The independent native-client diagnostic boundary remains unchanged. Its verifier receives only a compatibility marker refresh for the already-existing server-generated correlation argument.

## Static evidence

`scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs` guards:

- type-only context and runtime errors;
- bounded tenant, channel, slug, and locale shape facts;
- absence of raw diagnostic identities and complete error payloads;
- the server-generated correlation id;
- unchanged public code-message envelope;
- endpoint, dependency composition, command payload, validation messages, DTO mapping, and outer transport behavior;
- source-only validation flags and empty execution evidence.

## Remaining gaps

Compilation, mounted parity, native runtime, remote transport, browser evidence, compensation execution evidence, and the broader ecommerce mapper cleanup remain open.

## Suggested maintainer checks

```bash
node scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs
node scripts/verify/verify-order-storefront-native-client-diagnostics.mjs
node scripts/verify/verify-order-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order-storefront --all-features
```

These commands were intentionally not run by the implementation agent.
