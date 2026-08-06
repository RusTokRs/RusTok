# Commerce mounted storefront runtime error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the public error-envelope gap in
`crates/rustok-commerce/src/storefront_checkout_runtime_mounted.rs`.

The private compatibility implementation in
`crates/rustok-commerce/src/storefront_checkout_runtime.rs` still converts owner
and persistence failures into a legacy dynamic `Debug` message. The mounted
module previously re-exported that error type and the four non-checkout helper
functions directly, so a Rust consumer could receive the dynamic compatibility
message even though the current native payment and fulfillment transports
already replace it with static public text.

The mounted module now keeps the compatibility error private and exposes a
transport-owned static envelope for:

- storefront payment-collection reads;
- storefront refund-summary reads;
- storefront payment-collection creation;
- storefront shipping-selection updates.

Checkout completion remains on the existing staged owner-port pipeline and is
not changed by this source wave.

## Public envelopes

| Mounted operation | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Payment collection read | `STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE` | `Storefront payment collection is temporarily unavailable` | true |
| Refund summary read | `STOREFRONT_REFUND_SUMMARY_UNAVAILABLE` | `Storefront refund summary is temporarily unavailable` | true |
| Payment collection create | `STOREFRONT_PAYMENT_COLLECTION_CREATE_FAILED` | `Storefront payment collection is temporarily unavailable` | true |
| Shipping selection update | `STOREFRONT_SHIPPING_SELECTION_FAILED` | `Shipping selection is temporarily unavailable` | true |

`StorefrontCheckoutRuntimeError` exposes `public_message()`,
`public_code()`, and `retryable()` without retaining the private compatibility
message.

## Bounded diagnostics

The mounted facade logs only:

- a diagnostic token whose `Debug` output is always `redacted`;
- the static compatibility error type name;
- the mounted operation;
- boolean/non-nil shapes for tenant, authentication, and resource identity;
- request-context presence;
- channel id presence/non-nil shape;
- channel slug presence and length;
- locale presence and length;
- the selected public code and retryability.

It does not format, retain, log, or publish the private legacy error. Raw
database, owner, SDK, provider, metadata, cart, order, customer, tenant, actor,
channel, and locale values are not copied into the mounted diagnostic event.

## Preserved contracts

- The compatibility source file is unchanged and remains private to the mounted
  module.
- The same four legacy operations are delegated with the same runtime, tenant,
  request context, authentication context, resource ids, commands, and success
  DTOs.
- Payment collection, refund, and shipping native adapters continue to publish
  their existing static transport messages.
- Mounted checkout completion continues to call
  `storefront_staged_checkout_runtime::complete_storefront_checkout`.
- No owner port, persistence, GraphQL field, REST route, native command, DTO, or
  FFA/FBA status is changed.
- The broad ecommerce mapper and compatibility-source cleanup remain open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/storefront-checkout-runtime-error-safety-source-review.json`
- `scripts/verify/verify-commerce-storefront-checkout-runtime-error-safety.mjs`

## Still open

- Replace the private compatibility runtime with typed owner ports and remove it
  after compile, replay, mounted parity, and upgraded-path evidence.
- Continue order, payment execution/compensation, fulfillment, promotion,
  remaining adapter, and non-`PortError` envelope cleanup.
- Execute the focused verifier, Cargo checks, and mounted payment/refund/shipping
  scenarios.

## Intended checks

```bash
node scripts/verify/verify-commerce-storefront-checkout-runtime-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-payment-storefront --features ssr
cargo check -p rustok-fulfillment-storefront --features ssr
```

No tests, Node verifiers, Cargo commands, formatting, native server functions,
HTTP requests, workflows, or CI were executed.
