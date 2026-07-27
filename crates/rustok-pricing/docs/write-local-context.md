# Pricing write local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice retains stable owner-local outcome context for canonical root pricing writes:

- `PricingWritePort::upsert_variant_price`;
- `PricingWritePort::set_price_list_scope`;
- `PricingWritePort::apply_variant_discount`;
- `PricingWritePort::set_price_list_percentage_rule`;
- root `InProcessPricingWritePort`;
- root `in_process_pricing_write_port`.

The existing owner implementation in `ports.rs` remains unchanged. The root wrapper retains the delegated
`PortContext` and safe request facts, calls the original `PricingService` port implementation, classifies
covered returned `PortError` envelopes, normalizes identifying messages, and preserves kind, code, and
retryability.

## Canonical root cutover

The crate keeps `pub mod ports`, so the existing traits, requests, owner implementation, and module-path
factories remain available for compatibility. Root construction is now split explicitly:

- `InProcessPricingReadPort` owns canonical read construction;
- `InProcessPricingWritePort` owns canonical write construction;
- `rustok_pricing::ports` remains an explicit compatibility path.

The commerce GraphQL admin pricing mutations import the root write factory, so variant-price upsert,
discount application, price-list rule update, and price-list scope update use this wrapper without resolver
changes.

## Delegation order

Each wrapper operation performs the same sequence:

1. clone the accepted `PortContext` for diagnostics;
2. retain operation-specific typed identity and bounded shape facts;
3. delegate the original context and request to the unchanged owner implementation;
4. inspect only a returned `PortError`;
5. emit a local event only for a covered owner code;
6. return the same envelope or the same kind/code/retryability with a stable message.

The persistent owner continues to own write/idempotency admission, tenant and actor parsing, validation,
price-list and variant mutation, event publication, saved projections, and public error selection.

## Safe request facts

Covered diagnostics may retain:

- typed variant, price-list, and channel ids;
- minimum and maximum quantity bounds;
- currency-code, channel-slug, and fallback-locale character lengths;
- whether compare-at amount or adjustment percent was supplied.

The wrapper does not retain raw currency codes, channel slugs, fallback locales, handles, SKUs, prices,
compare-at prices, percentages, titles, returned rows, or owner error text.

## Covered stable outcomes

The wrapper recognizes stable owner codes and preserves existing kind and retryability.

| Stable code | Local operation | Canonical message policy |
| --- | --- | --- |
| `pricing.tenant_id_invalid` | `validate_tenant_context` | `pricing request context is invalid` |
| `pricing.actor_id_invalid` | `validate_actor_context` | `pricing write actor is invalid` |
| `pricing.database_unavailable` | `owner_storage` | unchanged shared unavailable message |
| `pricing.product_not_found` | `load_product` | `product was not found` |
| `pricing.variant_not_found` | `load_variant` | `variant was not found` |
| `pricing.duplicate_handle` | `validate_handle_uniqueness` | `pricing handle is already in use` |
| `pricing.duplicate_sku` | `validate_sku_uniqueness` | `pricing SKU is already in use` |
| `pricing.validation` | `validate_owner_request` | unchanged stable validation message |
| `pricing.insufficient_inventory` | `validate_inventory_requirement` | stable non-quantified conflict message |
| `pricing.invalid_option_combination` | `validate_option_combination` | unchanged stable validation message |
| `pricing.shipping_profile_not_found` | `load_shipping_profile` | `shipping profile was not found` |
| `pricing.duplicate_shipping_profile_slug` | `validate_shipping_profile_slug_uniqueness` | stable non-identifying conflict message |
| `pricing.no_variants` | `validate_product_variants` | unchanged stable validation message |
| `pricing.cannot_delete_published` | `validate_published_product_state` | unchanged stable conflict message |
| `pricing.rich_error` | `owner_rich_invariant` | unchanged shared invariant message |
| `pricing.core_error` | `owner_core_invariant` | unchanged shared invariant message |

Shared `port.*` admission envelopes and unknown future codes pass through without duplicate local
classification. Admission-specific pricing diagnostics remain separate follow-up work.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_pricing`;
- exact public owner operation and local operation;
- boundary `pricing_write_port`;
- correlation and tenant identity;
- typed actor, channel, locale, claims, and roles shape;
- causation id, traceparent, idempotency key, and deadline when available;
- safe operation-specific request facts;
- stable internal code and canonical public message;
- original message character length without message content;
- typed error kind, retryability, and the mapped public-safe `PortError`.

Unavailable, timeout, and invariant outcomes use error severity. Ordinary validation, not-found, and
conflict outcomes use warning severity.

## Preserved behavior

This work does not change:

- `PricingWritePort`, request DTOs, or returned owner projections;
- `PricingService` mutation methods or event publication;
- write/idempotency/deadline admission order;
- tenant and actor parsing;
- price, compare-at, tier, channel-scope, percentage, or locale semantics;
- GraphQL mutation arguments, permissions, public GraphQL envelopes, or success responses;
- read construction or read diagnostics;
- FBA or FFA status.

## Static evidence

`scripts/verify/verify-pricing-write-local-context.mjs` guards canonical construction, four unchanged owner
delegations, complete delegated context, safe request facts, stable message normalization, technical versus
ordinary severity, mapped error return, and the absence of raw pricing payload logging.

No verifier, formatting, Cargo, test, runtime, or remote-profile command was executed in this source wave.
