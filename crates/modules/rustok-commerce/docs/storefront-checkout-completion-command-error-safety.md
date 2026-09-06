# Commerce storefront checkout completion command error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the Commerce-owned wrapper:

- `complete_storefront_checkout`;
- `crates/rustok-commerce/storefront/src/transport/mod.rs`;
- `crates/rustok-commerce/storefront/src/transport/checkout_completion_command_error_safety.rs`.

The aggregate read, payment-collection command, and shipping-selection command policies remain unchanged.

After this slice, no Commerce storefront command wrapper remains on the generic mapper. The broad ecommerce correlation-safe mapper cleanup remains open for other module adapters and non-`PortError` public envelopes.

## Confirmed gap

The Order storefront owner returns a `UiTransportError` from its selected native or GraphQL transport. Commerce previously called:

```rust
complete_checkout(request).await.map_err(ApiError::from)
```

The generic `From<UiTransportError>` implementation copied `UiTransportError::to_string()` into the public `ApiError`. That display includes the failed transport path and the nested owner error text.

The Order owner already bounds technical failures:

- typed GraphQL failures become static checkout messages;
- native server-function failures become `Checkout transport is temporarily unavailable`;
- request validation is represented by bounded cart-ID and idempotency-key messages.

The final Commerce wrapper nevertheless republished the complete transport display rather than applying its own public policy.

## Implementation

`CheckoutCompletionCommandErrorContext` is created before the Order owner call. Each command receives a unique correlation id in the namespace:

```text
commerce-storefront-checkout:complete_storefront_checkout:<uuid>
```

After `complete_checkout`, the final `UiTransportError` is mapped by the Commerce-owned context.

The generic `impl From<UiTransportError> for ApiError` was removed from the Commerce storefront facade because no remaining Commerce owner wrapper uses it.

### Public policy

| Condition | Public `ApiError` |
| --- | --- |
| Recognized cart UUID or idempotency validation | `Validation("Invalid checkout request")` |
| Bounded native `Checkout request is invalid` validation | `Validation("Invalid checkout request")` |
| Native owner failure | `ServerFn("Checkout completion is temporarily unavailable")` |
| GraphQL owner failure | `Graphql("Checkout completion is temporarily unavailable")` |

Recognized validation envelopes are limited to:

- `cart_id must be a valid UUID`;
- `checkout idempotency key must contain 1 to 191 bytes`;
- `Checkout request is invalid`.

Arbitrary nested transport text is never forwarded to the public result.

## Internal diagnostics

The original `UiTransportError` is retained only as a private structured tracing field. The event also records:

- Commerce owner and operation;
- unique correlation id;
- whether a tenant slug is configured and its character length;
- cart-id character length;
- idempotency-key character length;
- source-module, source-surface, command, and owner-module character lengths;
- the `create_fulfillment` boolean;
- failed transport path;
- fallback flag;
- stable internal code;
- boundary name.

The structured request fields do not contain the tenant slug, cart ID, idempotency key, or command metadata values. The private error field may retain owner details for operators but is never copied into the public `ApiError`.

## Preserved behavior

This slice does not change:

- the Commerce `CheckoutCompletionCommandRequest` alias;
- the Order `CompleteCheckoutRequest` DTO;
- cart-ID normalization;
- idempotency-key generation;
- Order-owned command metadata;
- the GraphQL mutation, variables, or response mapping;
- typed GraphQL error classification;
- the native server function;
- request, tenant, and authentication context extraction;
- staged checkout orchestration;
- payment, order, fulfillment, and adjustment result mapping;
- native versus GraphQL feature selection;
- fallback behavior;
- aggregate Commerce reads;
- payment collection creation;
- shipping option selection;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Commerce wrapper state

All four Commerce storefront entry points now use boundary-owned public error policies:

- aggregate storefront fetch;
- payment collection creation;
- shipping option selection;
- checkout completion.

The facade no longer contains:

```rust
impl From<UiTransportError> for ApiError
```

and contains no `.map_err(ApiError::from)` owner-wrapper call sites.

## Static evidence

Focused source evidence:

- `crates/rustok-commerce/contracts/evidence/storefront-checkout-command-error-safety-source.json`;
- `crates/rustok-commerce/contracts/evidence/storefront-checkout-command-error-safety-source-review.json`;
- `scripts/verify/verify-commerce-storefront-checkout-command-error-safety.mjs`.

The focused verifier is imported by:

- `scripts/verify/verify-commerce-storefront-transport-error-safety.mjs`.

The prior aggregate, payment-command, and shipping-command guards now require zero generic Commerce owner mappings and the absence of the generic `From<UiTransportError>` implementation.

All execution and runtime validation flags remain `false`. Source review alone does not prove native, GraphQL, browser, mounted, workflow, CI, or production behavior.

## Remaining work

The master correlation-safe mapper cleanup remains open for:

- Pricing storefront and other remaining ecommerce adapters;
- remaining non-`PortError` public envelopes;
- any runtime verification and deployment evidence owned by maintainers.

Closing the Commerce storefront wrapper subset does not close the broad ecommerce plan item.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-checkout-command-error-safety.mjs
node scripts/verify/verify-commerce-storefront-shipping-command-error-safety.mjs
node scripts/verify/verify-commerce-storefront-payment-command-error-safety.mjs
node scripts/verify/verify-commerce-storefront-aggregate-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-handoff.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce-storefront
cargo check -p rustok-commerce-storefront --features hydrate
cargo check -p rustok-commerce-storefront --features ssr
```
