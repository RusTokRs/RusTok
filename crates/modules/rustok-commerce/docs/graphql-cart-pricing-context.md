# GraphQL storefront cart pricing context

Status: `source_ready_unvalidated`

This slice continues the still-open ecommerce public-error mapper cleanup without changing
FBA/FFA status.

## Closed source gap

The GraphQL storefront cart resolver and repricers retain the incoming request correlation,
attach the Cart/line operation as causation, and delegate through `PricingReadPort`.
Before these slices, returned `PortError` values reached the shared public cart mapper without
consumer-side access to the original tenant, channel, actor, locale, causation, correlation, or
exact pricing operation.

`safe_cart.rs` mounts one private `ContextualPricingReadPort` through a local `rustok_pricing`
import shim. All three pricing constructors (add, quantity and atomic context preparation) in the included
`cart.rs` resolver use that adapter.
`safe_legacy_helpers.rs` includes `helpers.rs` behind a second local
pricing shim and routes its `reprice_storefront_cart_line_items` constructor to the same adapter.
The canonical owner constructor remains
`rustok_pricing::in_process_pricing_read_port` and is called only inside the shared decorator.

The adapter implements the complete current `PricingReadPort` trait:

- resolve product price;
- resolve a bounded batch of product prices;
- list bounded storefront pricing summaries;
- read one price-list projection;
- list active price-list projections;
- read admin product pricing projection;
- read storefront product pricing projection;
- preview a variant discount.

Each failed pricing owner call records `owner = "rustok_pricing"`, correlation id, tenant,
channel, locale, actor kind/id, causation id, exact operation, typed owner code/kind, owner
retryability, and the `commerce_graphql_cart` boundary before returning the same `PortError` to
the existing public mapper.

## Preserved contracts

- Existing `CART_*` GraphQL messages and codes remain unchanged. Retryability comes from
  the owner, not from `Unavailable`/`Timeout` kind inference; permanent capability refusals
  remain non-retryable.
- Existing cart/pricing source-owner classification remains unchanged.
- Outer resolver/helper transport signatures, access checks, success responses and layered exports
  stay stable. Context mutation now sends one guarded Cart context command instead of two writes. Cart quantity edits and repricers use the shared Commerce quote
  adapter, not duplicated page-fallback request builders.
- Saved Cart channel is authoritative: no selectors means global; an ID-only selector never
  acquires a page slug. Missing locale inherits the admitted request; malformed saved locale
  refuses. Every quote is correlated before a price write.
- `cart.rs` and `helpers.rs` remain included through their safe facades. The actually mounted
  typed repricer supplies the selected runtime as the third argument to its aliased factory.
- The adapter neither mutates pricing requests nor relabels pricing errors as cart errors.
- Public buyer/cart transport types stay unchanged. The internal Cart context command now requires
  the original observation and explicit complete target-context pricing disposition.

## Still open

- Move original pre-`PortError` technical causes into correlation-aware owner-side pricing logging
  where a mapper does not already retain them.
- Review non-pricing legacy helper errors that still reach `legacy_graphql_error` only as
  `async_graphql::Error` values without typed owner context.
- Execute compile, static verifier, transport, and runtime evidence before changing any
  architecture status.

## Intended verification

```bash
node scripts/verify/verify-commerce-graphql-cart-pricing-context.mjs
node scripts/verify/verify-commerce-graphql-cart-owner-context.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
cargo check -p rustok-commerce --lib
```

The original source wave did not execute these commands. The 2026-10-30 targeted review runs
read-only source guards and formatting only; its [review record](implementation-plan.md#targeted-pricing-review-2026-10-30)
records actual results. Regression sources, Rust compilation and mounted behavior remain unrun.


## Atomic context continuation (2026-10-31)

Context preparation invokes selected `resolve_product_prices` batches before a single Cart command,
using original observation plus hypothetical target, saved Channel and owner-normalized locale.
The decorator retains each actual batch owner operation; the transport keeps `CART_REPRICE_FAILED`
and the exact retry bit without exposing owner text. There is no per-line fallback on batch refusal.
See the [handoff](implementation-plan.md#atomic-cart-context-adoption-2026-10-31). Source checks are
recorded there; regression sources and compiled/mounted/database/remote behavior remain unrun/unproven.
