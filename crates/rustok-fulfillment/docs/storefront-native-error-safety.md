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

Mounted tenant, authentication, optional request-context, and owner failures retain only bounded server-side diagnostics:

- fulfillment owner and exact owner operation;
- stable internal code and native boundary;
- the static Rust error type where an error value exists;
- tenant UUID non-nil state rather than the UUID value;
- request-context presence;
- correlation id when optional `RequestContext` extraction succeeds;
- channel UUID presence/non-nil state;
- channel-slug presence and length;
- locale presence and length.

The complete framework and owner errors are not logged, and tenant and request-context identity values are not logged: tenant UUID, channel UUID, channel slug, and locale remain outside structured diagnostic values.

Public `ServerFnError` messages remain static for missing runtime composition, tenant/authentication context extraction, and shipping-selection owner runtime failure. `RequestContext` remains optional for the owner call; failed extraction is recorded with the error type and bounded tenant/request-context facts and still results in `None`.

Missing `TransactionalEventBus` diagnostics remain dependency-name only and keep the static shipping-selection availability envelope.

## Native client contract

The private native adapter performs the same fail-fast validation order before the server-function call:

1. cart UUID parsing;
2. `build_shipping_selection_updates` selection-plan validation;
3. selected shipping-option UUID parsing for the materialized updates.

These failures return `ShippingSelectionTransportError::Validation` with the existing messages. After that preflight succeeds, the adapter creates one correlation-aware context and maps any remaining native transport failure to:

`Shipping selection request could not be completed`

The complete compatibility error remains private to the client adapter's structured diagnostics. Diagnostics record only operation, correlation id, stable code, boundary, identifier lengths, delivery-group/available-option counts, and seller/option presence. Cart ids, profile slugs, seller ids, option ids, delivery-group values, and request payloads are not logged.

## Preserved behavior

This mounted diagnostic cleanup does not change:

- `select_shipping_option` public signature or `UiTransportError` result;
- `SelectShippingOptionRequest`, delivery-group, update, or response types;
- the `fulfillment/select-shipping-option` endpoint;
- `build_shipping_selection_plan` or update materialization;
- cart, selection-plan, and shipping-option validation messages;
- `StorefrontShippingSelectionCommand` or its payload;
- tenant/authentication extraction order or optional `RequestContext` fallback;
- `select_storefront_shipping_option` owner invocation;
- the mounted `ShippingSelectionTransportError::ServerFn(error.to_string())` compatibility wrapper;
- the storefront transport facade, native client adapter, GraphQL adapter, GraphQL error policy, or native/GraphQL selection;
- fallback policy;
- fulfillment FBA or FFA status.

## Static evidence

Mounted server-side guard:

- `scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs`.

It requires the four bound-free type-only error sites, bounded tenant/request-context facts, static public envelopes, unchanged endpoint and owner-call markers, and source-only validation flags. It rejects complete error payloads and full tenant/channel/slug/locale diagnostic values.

Native client guard:

- `scripts/verify/verify-fulfillment-storefront-native-client-error-safety.mjs`.

It locks validation ordering and messages, adapter context placement, the static technical envelope, private client compatibility diagnostics, safe request-shape fields, unchanged facade/GraphQL contracts, source evidence statuses, and the still-open broad ecommerce mapper cleanup.

Retained mounted source evidence:

- `crates/rustok-fulfillment/contracts/evidence/storefront-native-error-safety-source.json`.

No test, verifier, Cargo command, formatting command, mounted execution, workflow, CI job, or runtime trace was executed for this source slice.

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
