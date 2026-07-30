# Payment storefront native client error safety

Status: **source-ready / unvalidated**

## Boundary

The payment storefront facade keeps explicit native/GraphQL transport selection for:

- `create_payment_collection`;
- `fetch_payment_collection`;
- `fetch_refund_summary`.

The mounted native server functions already keep owner and framework causes in structured
server diagnostics and return bounded public messages. The remaining client-side gap was the
outer native result: `PaymentTransportError::ServerFn(String)` was passed directly into
`UiTransportError`, whose public envelope retains the selected transport error display.

## Source policy

Each native closure now creates `NativeClientErrorContext` before the unchanged adapter call.
The final native error is mapped before `execute_selected_transport` aggregates it.

`PaymentTransportError::Validation` remains unchanged so existing request validation messages
retain their public contract. Technical native variants return one static message:

`Payment storefront request could not be completed`

The original technical error is retained only in structured diagnostics with:

- owner and owner operation;
- per-call correlation ID;
- stable code and boundary;
- cart/order identifier presence and character length;
- command-metadata presence.

Cart IDs, order IDs, command metadata values, payment collection fields, refund data, GraphQL
payloads, tokens, and tenant values are not logged by this final client mapper.

## Preserved behavior

- `PaymentFacadeError = UiTransportError` is unchanged.
- Explicit native/GraphQL selection and no-fallback behavior are unchanged.
- Three GraphQL contexts and their existing error mappings are unchanged.
- Native adapters and mounted server functions are unchanged.
- Request/response DTOs and payment-owned command metadata are unchanged.
- Request builders still trim cart/order IDs.
- No Payment FFA/FBA status is promoted.
- The broad ecommerce mapper-cleanup result remains open.

## Evidence boundary

This change is source-only. It does not prove compilation, server-function registration,
hydrate or SSR behavior, browser envelopes, mounted runtime diagnostics, workflow checks, CI,
or production behavior.

Suggested maintainer checks:

```bash
node scripts/verify/verify-payment-storefront-native-client-error-safety.mjs
node scripts/verify/verify-payment-storefront-native-error-safety.mjs
node scripts/verify/verify-payment-storefront-graphql-error-safety.mjs
npm run verify:payment:storefront-boundary
cargo check -p rustok-payment-storefront
cargo check -p rustok-payment-storefront --features hydrate
cargo check -p rustok-payment-storefront --features ssr
```
