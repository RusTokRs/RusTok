# Pricing read local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice protects the canonical root construction for all six `PricingReadPort`
operations:

- `resolve_product_price`;
- `read_price_list_projection`;
- `list_active_price_list_projections`;
- `read_admin_product_pricing_projection`;
- `read_storefront_product_pricing_projection`;
- `preview_variant_discount`.

The owner implementation in `ports.rs` remains the persistence and domain-policy
delegate. `InProcessPricingReadPort` retains the delegated context and safe request
facts, invokes that implementation unchanged, classifies known owner envelopes, and
returns either the unchanged error or the same kind/code/retryability with a stable
non-identifying message.

## Canonical root cutover

Root `rustok_pricing::in_process_pricing_read_port` now constructs
`InProcessPricingReadPort`. Current commerce, checkout, GraphQL, REST, admin, and
storefront consumers importing the root factory therefore use the guarded path without
changing their requests or orchestration.

`rustok_pricing::ports::in_process_pricing_read_port` remains available as an explicit
compatibility path. It is not counted as covered by this source slice. The root write
factory remains unchanged because this PR is deliberately read-only.

## Preserved owner behavior

The wrapper does not change:

- `PortCallPolicy::read()` admission or deadline semantics;
- tenant parsing and tenant isolation;
- product/variant consistency checks;
- price-list, channel, locale, quantity, and fallback resolution;
- pricing service method selection or ordering;
- request and response DTOs;
- error kind, code, or retryability;
- `PricingWritePort` construction and execution;
- runtime evidence status or FBA/FFA status.

## Safe request facts

Diagnostics may retain typed identifiers and bounded shape facts required to locate a
failed owner call:

- product, variant, region, channel, and price-list UUIDs;
- selected price-list UUID;
- requested quantity;
- character lengths for currency code, channel slug, locale, fallback locale,
  storefront handle, and public channel slug.

The wrapper does not log raw storefront handles, SKUs, channel slugs, currency values,
discount percentages, prices, compare-at prices, titles, rows, or returned pricing
projections. It records only the original public-message character count, never the
original text.

## Stabilized messages

Known identifying owner envelopes keep their existing kind, code, and retryability but
receive stable messages at the canonical boundary:

| Code | Stable message |
| --- | --- |
| `pricing.tenant_id_invalid` | `pricing request context is invalid` |
| `pricing.variant_product_mismatch` | `variant does not belong to the requested product` |
| `pricing.price_not_found` | `price was not found` |
| `pricing.price_list_not_found` | `price list was not found` |
| `pricing.product_not_found` | `product was not found` |
| `pricing.variant_not_found` | `variant was not found` |
| `pricing.duplicate_handle` | `pricing handle is already in use` |
| `pricing.duplicate_sku` | `pricing SKU is already in use` |
| `pricing.insufficient_inventory` | `inventory is insufficient for the pricing operation` |
| `pricing.shipping_profile_not_found` | `shipping profile was not found` |
| `pricing.duplicate_shipping_profile_slug` | `shipping profile slug is already in use` |

Already-stable validation, conflict, unavailable, and invariant envelopes are recorded
without changing their message. Unknown errors and shared `port.*` admission errors pass
through without an additional local event.

## Diagnostic context

Covered outcomes record:

- truthful owner `rustok_pricing`;
- exact public and local operation names;
- boundary `pricing_read_port`;
- correlation, tenant, actor, channel, locale, causation, trace, idempotency, and
  deadline context;
- claim and role counts rather than claim contents;
- safe request facts;
- stable code, safe public message, message length, kind, and retryability.

Unavailable, timeout, and invariant outcomes use error severity. Validation, not-found,
and conflict outcomes use warning severity.

## Static evidence

`scripts/verify/verify-pricing-read-local-context.mjs` locks:

- the root factory cutover and unchanged compatibility/write paths;
- all six unchanged owner delegations;
- complete context and safe request-fact retention;
- stable message replacement with preserved kind/code/retryability;
- technical-versus-ordinary severity;
- absence of raw pricing payload logging.

Intended focused verification:

```bash
node scripts/verify/verify-pricing-read-local-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-pricing --lib
cargo check -p rustok-commerce --lib
```

These commands are not executed in this source wave; validation remains maintainer-owned.
