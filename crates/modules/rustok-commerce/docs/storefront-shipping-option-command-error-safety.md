# Commerce storefront shipping option command error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the Commerce-owned wrapper:

- `select_storefront_shipping_option`;
- `crates/rustok-commerce/storefront/src/transport/mod.rs`;
- `crates/rustok-commerce/storefront/src/transport/shipping_option_command_error_safety.rs`.

The checkout-completion wrapper remains open on the generic `From<UiTransportError>` mapper. The aggregate read and payment-collection command policies remain unchanged.

## Confirmed gap

The Fulfillment storefront owner already applies bounded public messages to technical GraphQL and native failures. Its facade nevertheless returns `UiTransportError`, and Commerce previously called:

```rust
.map_err(ApiError::from)
```

The generic conversion publishes `UiTransportError::to_string()`. That display includes the failed transport path and nested owner error text.

Fulfillment validation also intentionally carries useful owner detail. Current messages include:

- `cart_id must be a valid UUID`;
- `selected_shipping_option_id must be a valid UUID`;
- a missing delivery group message containing shipping profile and seller identifiers;
- an unavailable option message containing option and shipping profile identifiers.

Those messages remain available inside the owner boundary, but they are not suitable as the final Commerce public envelope.

## Implementation

`ShippingOptionCommandErrorContext` is created before the Fulfillment owner call. Each command receives a unique correlation id in the namespace:

```text
commerce-storefront-shipping:select_storefront_shipping_option:<uuid>
```

After `select_shipping_option`, the final `UiTransportError` is mapped by the Commerce-owned context instead of the generic `ApiError::from` implementation.

### Public policy

| Condition | Public `ApiError` |
| --- | --- |
| Recognized cart, option, delivery-group, or availability validation | `Validation("Invalid shipping selection")` |
| Native owner failure | `ServerFn("Shipping selection is temporarily unavailable")` |
| GraphQL owner failure | `Graphql("Shipping selection is temporarily unavailable")` |

The validation classifier recognizes only the existing owner validation contracts. Arbitrary nested transport text is not forwarded to the public result.

## Internal diagnostics

The original `UiTransportError` is retained only as a private structured tracing field. The event also records:

- Commerce owner and operation;
- unique correlation id;
- whether a tenant slug is configured and its character length;
- cart id character length;
- delivery-group count;
- total available-option count;
- target shipping-profile character length;
- seller and shipping-option presence and character lengths;
- failed transport path;
- fallback flag;
- stable internal code;
- boundary name.

The structured request fields do not contain tenant slug, cart id, shipping profile slug, seller id, shipping option id, delivery groups, or available option id values. The private error field may retain owner details for operators but is never copied into the public `ApiError`.

## Preserved behavior

This slice does not change:

- the Commerce `SelectShippingOptionRequest` wrapper;
- the Fulfillment `SelectShippingOptionRequest` DTO;
- request normalization;
- delivery-group matching and selection planning;
- the GraphQL mutation, variables, or response mapping;
- the native server function;
- tenant, authentication, request-context, and owner-runtime mapping;
- native versus GraphQL feature selection;
- fallback behavior;
- aggregate Commerce reads;
- payment collection creation;
- checkout completion;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

Focused source evidence:

- `crates/rustok-commerce/contracts/evidence/storefront-shipping-command-error-safety-source.json`;
- `crates/rustok-commerce/contracts/evidence/storefront-shipping-command-error-safety-source-review.json`;
- `scripts/verify/verify-commerce-storefront-shipping-command-error-safety.mjs`.

The focused verifier is imported by:

- `scripts/verify/verify-commerce-storefront-transport-error-safety.mjs`.

The prior aggregate and payment-command guards now require exactly one remaining generic wrapper: checkout completion.

All execution and runtime validation flags remain `false`. Source review alone does not prove native, GraphQL, browser, mounted, workflow, CI, or production behavior.

## Remaining work

The master correlation-safe mapper cleanup remains open for:

- `complete_storefront_checkout` final Commerce public envelope;
- Pricing storefront and other remaining ecommerce adapters;
- remaining non-`PortError` public envelopes.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
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
