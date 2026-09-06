# Pricing read local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice closes the canonical local-diagnostic surface for all six `PricingReadPort`
operations:

- `resolve_product_price`;
- `read_price_list_projection`;
- `list_active_price_list_projections`;
- `read_admin_product_pricing_projection`;
- `read_storefront_product_pricing_projection`;
- `preview_variant_discount`.

`InProcessPricingReadPort` still delegates every call to the same owner implementation in
`ports.rs`. Request and response DTOs, operation ordering, policy admission, price
resolution, projection selection, returned values, error kind, code, retryability, public
message mapping, and technical-versus-ordinary severity are unchanged.

## Canonical root path

Root `rustok_pricing::in_process_pricing_read_port` continues to construct
`InProcessPricingReadPort`. The explicit compatibility factory under
`rustok_pricing::ports` and the canonical write factory remain unchanged.

## Bounded delegated context

Covered local outcomes retain correlation ID, exact owner and local operation names, and
the stable `pricing_read_port` boundary. Delegated context is represented only by:

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

Quantity is represented only by presence, non-zero state, and negative state; exact
quantity values are not recorded. Currency, slug, locale, fallback-locale, handle, and
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

`crates/rustok-pricing/src/write_context.rs` remains open as the next Pricing diagnostic
slice. The broader ecommerce cleanup and all execution evidence also remain open.

## Static evidence

- `scripts/verify/verify-pricing-read-local-context.mjs`
- `crates/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source.json`
- `crates/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source-review.json`

Intended maintainer validation:

```bash
node scripts/verify/verify-pricing-read-local-context.mjs
cargo check -p rustok-pricing --lib
cargo check -p rustok-commerce --lib
```

No test, verifier, formatter, Cargo, workflow, CI, or mounted runtime command was executed
for this source contract.
