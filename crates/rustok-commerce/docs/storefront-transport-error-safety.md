# Commerce storefront transport error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the mounted Commerce storefront aggregate read in:

- `crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs`;
- `crates/rustok-commerce/storefront/src/transport/shared_adapter.rs`.

The aggregate still delegates cart reads to `rustok-cart-storefront` and payment collection reads to `rustok-payment-storefront`. It does not reconstruct owner services, change checkout DTOs, or change native/GraphQL transport selection.

## Delivered source contract

The SSR-native server function now maps invalid cart selection, cart transport failure, and payment transport failure through static public messages. Internal causes are retained only in structured server logs with:

- owner and owner operation;
- Commerce consumer operation and boundary;
- correlation id and tenant id;
- channel id, channel slug, and locale;
- stable internal code.

The shared transport mapper no longer exposes `UiTransportError::to_string()` through `ApiError::ServerFn` or `ApiError::Graphql`. It records only safe failed-path, fallback, owner, operation, and stable-code fields, never the raw transport cause, then returns a static cart or payment-collection message.

The existing cart UUID validation compatibility remains explicit and static.

## Preserved behavior

This slice does not change:

- `FetchCommerceRequest`;
- `StorefrontCommerceData` or `StorefrontCheckoutWorkspace`;
- cart/payment owner request DTOs;
- cart/payment owner response mapping;
- native versus GraphQL selection policy;
- checkout commands or staged checkout orchestration;
- FBA or FFA status.

## Static evidence

`scripts/verify/verify-commerce-storefront-transport-error-safety.mjs` guards:

- the storefront package diagnostics dependency;
- native correlation/tenant/channel/locale diagnostics;
- cart and payment owner identities and operations;
- stable codes and static public messages;
- removal of foreign adapter `to_string()` public mapping;
- absence of raw causes from shared/client-side tracing;
- unchanged source-only validation flags.

## Remaining gaps

The master mapper-cleanup task remains open for other promotion consumers, compensation adapters, remaining ecommerce transports, and non-`PortError` public envelopes. Runtime, native/GraphQL parity, remote transport, and compiled evidence are also still open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-transport-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-handoff.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce-storefront --all-features
```
