# Payment storefront native error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the payment-owned native server-function adapter in:

- `crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs`.

It covers refund-summary read, payment-collection read, and payment-collection create/reuse. The GraphQL adapter, public DTOs, transport selection, commerce runtime operations, and payment facade variants are unchanged.

## Delivered source contract

Each native operation now obtains a request context before tenant, authentication, runtime-composition, and owner calls. Internal failures are logged server-side with the available:

- payment owner and exact owner operation;
- correlation id and tenant id;
- channel id, channel slug, and locale;
- stable internal code and native boundary;
- original internal error where one exists.

Public `ServerFnError` messages are static for:

- request-context extraction;
- tenant-context extraction;
- authentication-context extraction;
- missing host-composed payment runtime dependencies;
- refund-summary owner failure;
- payment-collection read failure;
- payment-collection create/reuse failure.

The payment-collection read endpoint now extracts `RequestContext` solely to retain correlation-safe diagnostics. Its owner request and response remain unchanged.

## Preserved behavior

This slice preserves:

- all three `#[server]` endpoint paths;
- `read_storefront_order_refunds`;
- `read_storefront_payment_collection`;
- `create_storefront_payment_collection` and its metadata payload;
- UUID validation messages;
- the three outer `PaymentTransportError::ServerFn` wrappers;
- the payment GraphQL adapter and native/GraphQL selection policy;
- payment FBA/FFA status.

## Static evidence

`scripts/verify/verify-payment-storefront-native-error-safety.mjs` guards:

- the diagnostics dependency;
- exact operation and endpoint markers;
- context-aware extraction/runtime/owner mappings;
- correlation, tenant, channel, locale, code, owner, and boundary logs;
- static public messages;
- removal of raw `ServerFnError` mappings;
- unchanged facade wrapper count and source-only validation flags.

## Remaining gaps

The master ecommerce mapper-cleanup task remains open for payment compensation/execution consumers, other ecommerce transports, remaining owner adapters, and non-`PortError` public envelopes. Compile, mounted parity, remote transport, and runtime evidence are also still open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-storefront-native-error-safety.mjs
node scripts/verify/verify-payment-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment-storefront --all-features
```
