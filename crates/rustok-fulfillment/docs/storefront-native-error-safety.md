# Fulfillment storefront native error safety

Status: **source-ready / unvalidated**

## Scope

The fulfillment-owned native shipping-selection path has two source-only safety boundaries:

- mounted server-function safety in
  `crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs`;
- final native client-adapter safety in
  `crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/native_client_error_safety.rs`.

The mounted boundary covers host runtime dependency resolution, tenant and authentication context extraction, optional request-context diagnostics, and the Commerce checkout runtime call. The client boundary prevents an unexpected server-function or framework string from becoming the selected native-path text inside public `UiTransportError`.

## Mounted server-function contract

Internal failures are retained only in server-side diagnostics with the available:

- fulfillment owner and exact owner operation;
- tenant id;
- correlation id, channel id, channel slug, and locale when optional `RequestContext` extraction succeeds;
- stable internal code and native boundary;
- original internal error where one exists.

Public `ServerFnError` messages remain static for missing runtime composition, tenant/authentication context extraction, and shipping-selection owner runtime failure. `RequestContext` remains optional for the owner call; failed extraction is logged and still results in `None`.

## Native client contract

The private native adapter now performs the same fail-fast validation order before the server-function call:

1. cart UUID parsing;
2. `build_shipping_selection_updates` selection-plan validation;
3. selected shipping-option UUID parsing for the materialized updates.

These failures return `ShippingSelectionTransportError::Validation` with the existing messages. After that preflight succeeds, the adapter creates one correlation-aware context and maps any remaining native transport failure to:

`Shipping selection request could not be completed`

The complete compatibility error remains private to structured diagnostics. Diagnostics record only operation, correlation id, stable code, boundary, identifier lengths, delivery-group/available-option counts, and seller/option presence. Cart ids, profile slugs, seller ids, option ids, delivery-group values, and request payloads are not logged.

## Preserved behavior

This slice does not change:

- `select_shipping_option` public signature or `UiTransportError` result;
- `SelectShippingOptionRequest`, delivery-group, update, or response types;
- the `fulfillment/select-shipping-option` endpoint;
- `build_shipping_selection_plan` or update materialization;
- cart, selection-plan, and shipping-option validation messages;
- `StorefrontShippingSelectionCommand` or its payload;
- the mounted `ShippingSelectionTransportError::ServerFn(error.to_string())` compatibility wrapper;
- the storefront transport facade, GraphQL adapter, GraphQL error policy, or native/GraphQL selection;
- fallback policy;
- fulfillment FBA or FFA status.

## Static evidence

The retained server-side guard remains:

- `scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs`.

The new focused guard is:

- `scripts/verify/verify-fulfillment-storefront-native-client-error-safety.mjs`.

It locks validation ordering and messages, adapter context placement, the static technical envelope, private raw-cause diagnostics, safe request-shape fields, unchanged facade/GraphQL/server-function source, source evidence statuses, and the still-open broad ecommerce mapper cleanup.

## Remaining gaps

The master ecommerce mapper-cleanup task remains open for fulfillment execution/recovery, other owner adapters, and non-`PortError` public envelopes. Compile, hydrate/SSR, mounted parity, remote transport, browser, workflow, CI, and production evidence also remain open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-storefront-native-client-error-safety.mjs
node scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs
node scripts/verify/verify-fulfillment-storefront-graphql-error-safety.mjs
npm run verify:fulfillment:storefront-boundary
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-fulfillment-storefront --all-features
```
