# Fulfillment shipping-selection diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source slice closes the currently identified payload-diagnostic gap in the
native/FBA `ShippingSelectionPort` owner boundary implemented by
`crates/rustok-fulfillment/src/ports.rs`.

It covers:

- `list_seller_shipping_options`;
- `select_shipping_option`;
- tenant UUID parsing for both operations;
- all five current `FulfillmentError` variants.

The public trait, request/response DTOs, seller/profile filtering, owner service
calls, and Commerce/storefront composition remain unchanged.

## Safe context shape

Every retained event keeps the correlation id and static owner operation. Other
`PortContext` values are represented only by:

- tenant and actor-id character lengths;
- actor kind;
- claim and role counts;
- channel, causation-id, traceparent, and idempotency-key presence plus optional
  character lengths;
- locale character length;
- deadline milliseconds.

Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values
are not recorded.

Tenant UUID parse rejection retains only `tenant_id_parse_failed = true`, supplied
length, safe context shape, stable code, and boundary. The UUID parser error is not
formatted or retained.

## Owner error payload shape

All five `FulfillmentError` variants are classified through a closed static label:

- `validation`;
- `shipping_option_not_found`;
- `fulfillment_not_found`;
- `invalid_transition`;
- `database`.

Diagnostic evidence is limited to aggregate facts:

- text-field count and total character length;
- UUID-field count and non-nil count;
- opaque-payload presence for database errors.

Validation text, transition source/target text, resource UUIDs, database error
payloads, and complete `FulfillmentError` debug/display representations are not
recorded.

## Preserved behavior

This diagnostic-only change does not alter:

- read-policy admission for listing;
- write-policy and write-semantics admission for selection;
- tenant parsing order;
- locale arguments supplied to `FulfillmentService`;
- seller/profile filtering;
- shipping-option lookup or projection construction;
- database error severity;
- warning severity for validation, not-found, transition, and context rejection;
- public `PortError` codes, messages, kinds, or retryability;
- FFA/FBA status.

## Static evidence

- `crates/rustok-fulfillment/contracts/evidence/shipping-selection-diagnostic-safety-source.json`
- `crates/rustok-fulfillment/contracts/evidence/shipping-selection-diagnostic-safety-source-review.json`
- `scripts/verify/verify-fulfillment-shipping-selection-diagnostic-safety.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`

## Remaining gaps

The shipping-selection owner payload sites are **source-closed / unvalidated**.
Shipping-option projection read diagnostics in `shipping_option_read.rs` and
fulfillment lifecycle read diagnostics in `fulfillment_read.rs` remain separate
bounded Fulfillment slices. Checkout execution, remaining ecommerce owners and
adapters, non-`PortError` envelopes, compile, runtime, restart, contention, and
remote evidence also remain open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-shipping-selection-diagnostic-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs
npm run verify:fulfillment:storefront-boundary
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
```
