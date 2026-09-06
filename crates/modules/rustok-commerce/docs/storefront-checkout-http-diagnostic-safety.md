# Storefront checkout HTTP diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

This source slice hardens the two public HTTP error mappers in
`crates/rustok-commerce/src/controllers/store/checkout.rs`:

- `storefront_checkout_http_error`, which maps the staged storefront checkout runtime envelope;
- `payment_collection_http_error`, which maps direct storefront payment-collection owner errors.

Both mappers already returned stable public `HttpError` envelopes, but their private diagnostics
recorded complete error payloads and raw tenant, actor, cart, customer, channel, and locale values.

## Checkout runtime mapper

`StorefrontStagedCheckoutRuntimeError` has seven closed variants. The mapper now records only:

- the stable closed variant;
- text-field count and aggregate character length for `Validation`;
- boolean/non-nil identity shape for tenant, actor, and cart;
- channel-ID presence/non-nil shape;
- channel-slug presence and length;
- locale length;
- the static route operation;
- policy error kind, public code, retryability, numeric HTTP status, and boundary.

It does not record the complete runtime error, validation text, UUID values, channel slug, or
locale value.

The existing status/code/message/retryability policy is unchanged.

## Payment collection mapper

`PaymentError` has eleven variants. The mapper now records only:

- the stable closed variant;
- text-field count and aggregate character length;
- UUID-field count and non-nil count;
- whether an opaque database payload exists;
- boolean/non-nil identity shape for tenant, actor, cart, and optional customer;
- bounded channel and locale shape;
- the static owner operation;
- policy error kind, public code, numeric HTTP status, and boundary.

It does not record the complete `PaymentError`, database payload, validation text, transition
values, provider IDs, provider operation text, collection/payment/refund UUIDs, or raw route
context.

The existing public payment status, code, message, and route behavior are unchanged.

## Preserved behavior

This source slice does not change:

- storefront channel admission;
- customer/cart access checks;
- repricing and context resolution;
- reusable payment collection lookup;
- payment collection creation;
- staged checkout orchestration;
- request forwarding;
- idempotency-key validation;
- checkout runtime public code/message/retryability selection;
- payment collection HTTP policy;
- response types or OpenAPI declarations;
- Commerce or Payment FFA/FBA status.

## Remaining work

The master ecommerce correlation-safe mapper-cleanup item remains open. Separate public-envelope
and adapter diagnostics remain across other commerce routes and owner modules.

## Evidence

- `crates/rustok-commerce/contracts/evidence/storefront-checkout-http-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-storefront-checkout-http-error-context.mjs`

Evidence is source-only. The execution list is empty and every validation flag remains false.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-checkout-http-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
