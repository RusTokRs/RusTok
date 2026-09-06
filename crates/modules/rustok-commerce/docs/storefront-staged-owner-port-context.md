# Storefront staged checkout owner-port context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the owner-attribution and retained-context gap for the two
owner-port reads mounted directly in
`crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs`:

- `CartStorefrontPort::read_storefront_cart`;
- `CustomerReadPort::read_customer_projection_by_user`.

Both callsites already retained and cloned a `PortContext`. Before this slice, the
shared runtime mapper received that context and the exact operation, but did not
receive truthful owner identity. Its diagnostics recorded only correlation id,
tenant id, owner code, and typed kind, used error severity for every failure, and
did not record actor, channel, locale, deadline, internal message, retryability, or
the selected public runtime outcome.

The shared storefront lookup in `controllers/store/mod.rs`, HTTP transport mappers,
checkout stage internals, payment execution/compensation consumers, and other owner
adapters remain separate concerns.

## Delivered source contract

The cart read now passes:

- truthful owner `rustok_cart`;
- exact operation `read_storefront_cart`;
- the existing retained cart `PortContext`;
- the existing `CartAccess` fallback.

The authenticated customer read now passes:

- truthful owner `rustok_customer`;
- exact operation `read_customer_projection_by_user`;
- the existing retained customer `PortContext`;
- the existing `CartAccess` fallback.

The shared owner-port mapper now:

1. selects the same public runtime outcome before diagnostics;
2. maps only `Unavailable` and `Timeout` to `TemporarilyUnavailable`;
3. preserves the supplied fallback for every other owner error kind;
4. records the complete available owner and delegated context;
5. returns the already selected public runtime outcome after diagnostics.

Diagnostics record:

- truthful owner and exact owner operation;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- original owner code, internal message, typed kind, and retryability;
- mapped public runtime code and retryability;
- boundary `commerce_storefront_staged_checkout_runtime`.

Unavailable, timeout, and invariant failures use error severity. Ordinary owner
rejections use warning severity. Invariant failures retain their pre-existing
fallback outcome; severity classification does not change public behavior.

## Preserved behavior

This slice does not change:

- public `StorefrontStagedCheckoutRuntimeError` variants;
- public codes, messages, or retryability;
- idempotency-key validation;
- cart request identity or retained cart context construction;
- cart locale, country, and region fallback;
- customer context construction or two-second deadline;
- anonymous-customer behavior;
- `customer.customer_by_user_not_found -> Ok(None)` behavior;
- customer-owned cart access checks;
- actor-id resolution;
- pricing resolver or atomic-cart composition;
- inventory reservation identity wiring;
- checkout plan, marketplace, payment-provider, staged, compensation, or recovery
  composition;
- recovery error mapping for reconciliation, compensation pending, journal, or
  staged failures;
- REST, GraphQL, native, or mounted facade delegation;
- FBA, FFA, or ecommerce audit status.

## Static evidence

`scripts/verify/verify-commerce-storefront-staged-owner-context.mjs` guards:

- stable cart/customer owner constants and runtime boundary;
- one truthful cart owner callsite and one truthful customer owner callsite;
- retained context clones and exact operations;
- unchanged cart/customer fallback inputs;
- mapper inputs for context, owner, operation, original error, and fallback;
- unchanged `Unavailable | Timeout -> TemporarilyUnavailable` classification;
- unchanged fallback behavior for all other kinds;
- technical versus ordinary rejection severity;
- complete available context and owner error fields;
- mapped public code/retryability before returning the selected outcome;
- unchanged customer-not-found behavior;
- unchanged checkout composition and recovery mapper;
- absence of the two old owner-free mapper call forms.

The existing
`scripts/verify/verify-commerce-storefront-staged-checkout-cutover.mjs` remains
structurally compatible because mounted delegation, checkout composition, public
runtime error contracts, and recovery diagnostics are unchanged.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- the shared storefront customer lookup in `controllers/store/mod.rs`;
- remaining payment execution and compensation consumers;
- remaining order, fulfillment, inventory, customer, tax, promotion, and ecommerce
  adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-storefront-staged-owner-context.mjs
node scripts/verify/verify-commerce-storefront-staged-checkout-cutover.mjs
node scripts/verify/verify-commerce-storefront-checkout-http-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
