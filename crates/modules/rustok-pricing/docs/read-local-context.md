# Pricing read local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice closes the canonical local-diagnostic surface for all eight `PricingReadPort`
operations:

- `resolve_product_price`;
- `resolve_product_prices`;
- `list_storefront_product_pricing`;
- `read_price_list_projection`;
- `list_active_price_list_projections`;
- `read_admin_product_pricing_projection`;
- `read_storefront_product_pricing_projection`;
- `preview_variant_discount`.

`InProcessPricingReadPort` still delegates every call to the same owner implementation in
`ports.rs`. The 2026-10-29 continuation extends owner DTOs and resolution with batch
quotes and a catalog projection. The wrapper remains diagnostic-only: it forwards
context/requests without a second resolver and preserves owner kind/code/retryability.
Static public presentation and technical-versus-ordinary severity remain unchanged.

## Canonical root path

Root `rustok_pricing::in_process_pricing_read_port` continues to construct
`InProcessPricingReadPort`. The explicit compatibility factory under
`rustok_pricing::ports` and the canonical write factory remain unchanged.

## Bounded delegated context

Covered local outcomes retain correlation ID, exact owner and local operation names, and
the stable `pricing_read_port` boundary. The bounded delegated context is represented only by:

- tenant and actor-ID character lengths;
- a closed actor-kind label;
- claim and role counts;
- channel presence and character length;
- locale character length;
- causation, traceparent, and idempotency presence and character length;
- deadline value.

Raw tenant, actor, channel, locale, causation ID, traceparent, and idempotency-key values
are not recorded.

## Bounded request shape

Request identity is retained only as UUID presence and non-nil state. Product, variant,
region, channel, price-list, and selected-price-list UUID values are not recorded.

The bounded SKU-set size is recorded as a count. Quantity is represented only by presence, non-zero state, and negative state;
exact quantity values are not recorded. Currency, slug, locale, fallback-locale, handle, and
public-channel-slug inputs remain represented by character length only.

Raw storefront handles, channel slugs, currency codes, discount percentages, prices,
compare-at prices, titles, rows, and returned pricing projections are not recorded.

## Local error shape

Known owner envelopes keep the same classification and message mapping. Unknown errors
and shared `port.*` admission errors still pass through without an additional local event.

For covered outcomes diagnostics retain:

- the stable error code;
- a closed `PortErrorKind` label;
- retryability;
- original and public message character lengths;
- whether a public message is present.

The public message text is not recorded, the original message text is not recorded, and
`PortErrorKind` is not emitted through debug formatting.

Unavailable, timeout, and invariant outcomes remain error severity. Validation,
not-found, and conflict outcomes remain warning severity.

## Deliberate boundary

Both canonical Pricing wrappers are now source-closed for the previously identified local
payload-diagnostic gaps. The broader ecommerce cleanup and execution evidence remain open.

## Static evidence

- `scripts/verify/verify-pricing-read-local-context.mjs`
- `crates/modules/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source.json`
- `crates/modules/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source-review.json`

Intended maintainer validation:

```bash
node scripts/verify/verify-pricing-read-local-context.mjs
cargo check -p rustok-pricing --lib
cargo check -p rustok-commerce --lib
```

The original source snapshot claimed no test, verifier, formatter, Cargo, workflow, CI, or
mounted runtime execution. See the dated continuation below for current source-only checks;
historical evidence is not current execution evidence.


## Owner classification continuation (2026-10-18)

The wrapper remains a bounded delegated context adapter, not a second Foundation classifier.
The [owner continuation](./pricing-owner-port-error-safety.md#nested-error-and-public-consumer-continuation-2026-10-18)
now classifies nested Core/Rich errors by kind. Existing recognized outcomes keep their local
operation labels; new owner codes pass through unchanged when not recognized, without a new
local event. The canonical owner still emits its bounded payload facts. Wrapper delegation,
context, request shape, and mutation/persistence semantics are unchanged. Standalone source
verification was run for this continuation; tests, compilation and mounted evidence remain open.
Historical evidence is not upgraded.


## Quote contract continuation (2026-10-29)

The eight methods retain the canonical owner delegation. New batch/catalog diagnostic
facts contain counts and field presence/shape, not SKU IDs, prices or exact quantity.
The snapshot/source/context/time DTO extension is specified in [CRATE_API.md](../CRATE_API.md).
Tests, Cargo, SQL isolation and mounted evidence are unrun; formatting and scoped
non-test source/type checks are disclosed in the active implementation plan.
