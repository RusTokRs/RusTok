# Pricing write local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice closes the canonical local-diagnostic surface for all four `PricingWritePort`
operations:

- `upsert_variant_price`;
- `set_price_list_scope`;
- `apply_variant_discount`;
- `set_price_list_percentage_rule`.

`InProcessPricingWritePort` still delegates every call to the same owner implementation in
`ports.rs`. Request and response DTOs, operation ordering, write/idempotency/deadline
admission, tenant and actor parsing, mutation behavior, event publication, returned values,
error kind, code, retryability, public message mapping, and technical-versus-ordinary
severity are unchanged.

## Canonical root path

Root `rustok_pricing::in_process_pricing_write_port` continues to construct
`InProcessPricingWritePort`. The commerce GraphQL pricing mutations continue to use that
root factory. The explicit compatibility factory under `rustok_pricing::ports` and the
canonical read factory remain unchanged.

## Bounded delegated context

Covered local outcomes retain correlation ID, exact owner and local operation names, and
the stable `pricing_write_port` boundary. Delegated context is represented only by:

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

Request identity is retained only as UUID presence and non-nil state. Variant, price-list,
and channel UUID values are not recorded.

Minimum and maximum quantities are represented only by presence, non-zero state, and
negative state; exact minimum and maximum quantity values are not recorded. Currency,
channel-slug, and fallback-locale inputs remain represented by character length only.
Whether compare-at amount or adjustment percent was supplied remains a boolean presence
fact; their values are not recorded.

Raw currency codes, channel slugs, fallback locales, handles, SKUs, prices, compare-at
prices, percentages, titles, returned rows, and owner error text are not recorded.

## Local error shape

Known owner envelopes keep the same classification and message mapping. Shared `port.*`
admission envelopes and unknown future codes still pass through without an additional
local event.

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

## Pricing boundary status

The owner `ports.rs`, canonical read wrapper, and canonical write wrapper are now
source-closed for the currently identified public-message and payload-diagnostic gaps.
Both canonical Pricing wrappers are now source-closed. The broader ecommerce cleanup and
all execution evidence remain open.

## Static evidence

- `scripts/verify/verify-pricing-write-local-context.mjs`
- `crates/rustok-pricing/contracts/evidence/pricing-write-local-diagnostic-safety-source.json`
- `crates/rustok-pricing/contracts/evidence/pricing-write-local-diagnostic-safety-source-review.json`

Intended maintainer validation:

```bash
node scripts/verify/verify-pricing-write-local-context.mjs
node scripts/verify/verify-pricing-read-local-context.mjs
node scripts/verify/verify-pricing-owner-port-error-safety.mjs
cargo check -p rustok-pricing --lib
cargo check -p rustok-commerce --lib
```

No test, verifier, formatter, Cargo, workflow, CI, or mounted runtime command was executed
for this source contract.
