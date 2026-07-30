# Commerce storefront payment collection command error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the final Commerce public envelope for:

- `create_storefront_payment_collection`;
- `crates/rustok-commerce/storefront/src/transport/mod.rs`;
- `crates/rustok-commerce/storefront/src/transport/payment_collection_command_error_safety.rs`.

The shipping-selection and checkout-completion wrappers remain open and continue to use the generic `From<UiTransportError>` conversion.

## Confirmed gap

The Payment owner already bounds its failures:

- both native and GraphQL paths validate cart identity as `cart_id must be a valid UUID`;
- native runtime failures use static payment storefront messages;
- GraphQL network, HTTP, authentication, rejection, and unknown failures are mapped to static owner messages with private diagnostics.

The Commerce facade nevertheless called:

```rust
create_payment_collection(request)
    .await
    .map_err(ApiError::from)
```

The generic conversion used `UiTransportError::to_string()`, which includes transport-path metadata and the nested owner message. The final Commerce response therefore republished the full transport display instead of an operation-owned envelope.

## Implementation

`PaymentCollectionCommandErrorContext` is created before the Payment owner call. Every command receives a unique correlation id in the namespace:

```text
commerce-storefront-payment:create_storefront_payment_collection:<uuid>
```

The final `UiTransportError` is mapped structurally.

### Public policy

| Condition | Public `ApiError` |
| --- | --- |
| Native or GraphQL cart UUID validation | `Validation("Invalid cart selection")` |
| Native command failure | `ServerFn("Storefront payment collection is temporarily unavailable")` |
| GraphQL command failure | `Graphql("Storefront payment collection is temporarily unavailable")` |

The failed-path variant is preserved. No `UiTransportError` display value is copied into the public response.

## Internal diagnostics

The original `UiTransportError` remains available only as an internal tracing field. The event also records:

- Commerce owner and operation;
- unique correlation id;
- whether a tenant slug is configured and its character length;
- cart id character length;
- character lengths for the four payment command metadata fields;
- failed transport path;
- fallback flag;
- stable internal code;
- boundary name.

The structured fields do not include the tenant slug, cart id, source module, source surface, command, or owner module values.

## Preserved behavior

This slice does not change:

- `PaymentCollectionCreateRequest`;
- payment-owned command metadata;
- the Payment GraphQL mutation, variables, or response mapping;
- the Payment native server function;
- owner validation, authentication, or runtime policy;
- native versus GraphQL feature selection;
- fallback behavior;
- shipping option selection;
- checkout completion;
- FFA, FBA, browser, runtime, workflow, CI, or production status.

## Static evidence

Focused source evidence:

- `crates/rustok-commerce/contracts/evidence/storefront-payment-command-error-safety-source.json`;
- `crates/rustok-commerce/contracts/evidence/storefront-payment-command-error-safety-source-review.json`;
- `scripts/verify/verify-commerce-storefront-payment-command-error-safety.mjs`.

The focused verifier is imported by `scripts/verify/verify-commerce-storefront-transport-error-safety.mjs`. The earlier aggregate guard now requires exactly two remaining generic command mappings.

All execution and runtime validation flags remain `false`. Source review alone does not prove native, GraphQL, browser, mounted, workflow, CI, or production behavior.

## Remaining work

The broad correlation-safe mapper cleanup remains open for:

- `select_storefront_shipping_option` final Commerce public envelope;
- `complete_storefront_checkout` final Commerce public envelope;
- Pricing storefront and other ecommerce adapters;
- remaining non-`PortError` public envelopes.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-payment-command-error-safety.mjs
node scripts/verify/verify-commerce-storefront-aggregate-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-handoff.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce-storefront
cargo check -p rustok-commerce-storefront --features hydrate
cargo check -p rustok-commerce-storefront --features ssr
```
