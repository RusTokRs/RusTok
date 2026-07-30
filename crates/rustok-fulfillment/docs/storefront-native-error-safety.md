# Fulfillment storefront native error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the fulfillment-owned native shipping-selection server-function adapter in:

- `crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs`.

It covers host runtime dependency resolution, tenant and authentication context extraction, optional request-context diagnostics, and the Commerce checkout runtime call used to select a shipping option.

## Delivered source contract

Internal failures are retained only in server-side diagnostics with the available:

- fulfillment owner and exact owner operation;
- tenant id;
- correlation id, channel id, channel slug, and locale when optional `RequestContext` extraction succeeds;
- stable internal code and native boundary;
- original internal error where one exists.

Public `ServerFnError` messages are static for:

- missing `TransactionalEventBus` runtime composition;
- tenant-context extraction;
- authentication-context extraction;
- shipping-selection owner runtime failure.

`RequestContext` remains optional for the owner call. A failed extraction is logged and still results in `None`, preserving the previous runtime behavior.

## Preserved behavior

This slice does not change:

- `SelectShippingOptionRequest` or fulfillment storefront response types;
- the `fulfillment/select-shipping-option` endpoint;
- `build_shipping_selection_updates` validation messages;
- cart and shipping-option UUID validation messages;
- `StorefrontShippingSelectionCommand` or its payload;
- the outer `ShippingSelectionTransportError::ServerFn` wrapper;
- the GraphQL adapter or native/GraphQL transport selection;
- fulfillment FBA or FFA status.

## Static evidence

`scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs` guards:

- the storefront diagnostics dependency;
- exact endpoint and owner-operation markers;
- static runtime, context, and owner public envelopes;
- optional request-context preservation;
- correlation, tenant, channel, locale, owner, code, and boundary logs;
- removal of raw native owner/context error mapping;
- unchanged validation and outer transport semantics;
- source-only validation flags.

## Remaining gaps

The master ecommerce mapper-cleanup task remains open for other ecommerce transports, compensation/execution adapters, remaining owner consumers, and non-`PortError` public envelopes. Compile, mounted parity, remote transport, and runtime evidence are also still open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs
node scripts/verify/verify-fulfillment-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-fulfillment-storefront --all-features
```
