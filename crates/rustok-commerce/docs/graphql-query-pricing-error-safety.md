# Commerce GraphQL pricing error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the currently identified dynamic pricing `PortError.message` handling gap in the mounted Commerce GraphQL query facade.

The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. Its four pricing owner calls continue to serve:

- admin product pricing projection;
- storefront active price-list projections;
- storefront product pricing projection;
- per-variant effective-price resolution.

The existing pricing owner factory, `PricingReadPort` operations, request DTOs, `PortContext` construction, deadlines, actor/channel/locale context, successful owner snapshots, GraphQL fields, and output DTO conversions are unchanged.

## Typed facade

The mounted safe-query source aliases only the pricing dependency used by the unchanged compatibility query. The facade wraps the canonical `rustok_pricing::in_process_pricing_read_port` and delegates the same four owner operations.

The original resolver has two deliberate not-found branches:

- missing admin product pricing returns `None`;
- missing per-variant effective price leaves that variant price as `None`.

The facade preserves those branches through a closed local kind derived only from `PortErrorKind`. Owner code strings and owner message text are not used for control flow.

Active price-list and storefront product projection failures, plus every non-not-found admin/effective-price failure, retain the complete typed owner `PortError` until the transport mapper.

## Public envelopes

Transport responses are classified only by `PortErrorKind`.

| Owner kind | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `PRICING_REQUEST_INVALID` | `Pricing query is invalid` | false |
| NotFound | `PRICING_RESOURCE_NOT_FOUND` | `Pricing data was not found` | false |
| Conflict | `PRICING_STATE_CONFLICT` | `Pricing state conflicts with this query` | false |
| Forbidden | `PRICING_ACCESS_DENIED` | `Pricing query is not permitted` | false |
| Unavailable / Timeout | `PRICING_TEMPORARILY_UNAVAILABLE` | `Pricing data is temporarily unavailable` | true |
| InvariantViolation | `PRICING_OPERATION_FAILED` | `Pricing query could not be completed safely` | false |

The complete owner `PortError`, owner message content, owner code, and owner retryability are not copied into the GraphQL response.

## Bounded diagnostics

The transport boundary retains:

- a diagnostic token whose `Debug` output is always `redacted`;
- closed owner kind;
- stable owner code;
- owner retryability;
- owner-message presence and character length;
- selected public code and retryability;
- error severity for unavailable, timeout, and invariant failures;
- warning severity for validation, not-found, conflict, forbidden, and other ordinary rejections.

The complete `PortError` and owner-message content are not logged.

## Preserved contracts

- `rustok-pricing` remains the owner of pricing reads and error mapping.
- All four original owner calls and request values are unchanged.
- Admin and effective-price not-found behavior remains `None`.
- Storefront pricing projection keeps its owner-returned optional success shape.
- `query.rs` remains unchanged.
- GraphQL fields and DTOs are unchanged.
- Pricing and Commerce FFA/FBA status is unchanged.
- No compile, runtime, mounted GraphQL, remote-adapter, or parity evidence is claimed.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-pricing-error-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-pricing-error-safety.mjs`

## Still open

- Execute the focused source verifier and Cargo checks.
- Retain mounted success, not-found, validation, unavailable, timeout, and invariant GraphQL evidence.
- Continue cart, tax, promotion, inventory, remaining adapter, write-side, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-pricing-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-pricing --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
