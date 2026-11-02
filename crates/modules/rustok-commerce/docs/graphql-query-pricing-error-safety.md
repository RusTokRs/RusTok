# Commerce GraphQL pricing error safety

Status: `source_closed_unvalidated`

## Scope

The resolver uses bounded batch quotes and an owner catalog projection. Its five
Pricing owner call sites serve admin detail, active lists, the public atlas, public
detail and the complete bounded SKU quote set. The 2026-10-29 continuation extends
request/response DTOs with canonical Product membership, source, context and time;
admin tenant input is checked against the authenticated current tenant.

## Typed facade

The mounted safe-query dependency adapter wraps the canonical
`rustok_pricing::in_process_pricing_read_port`. It retains typed owner errors until
static transport presentation. Only typed `NotFound` with the closed owner code
`pricing.product_not_found` returns missing admin Product `None`; an invalid/foreign Price
List is a refusal, not a missing Product.
A valid unpriced SKU is now an explicit batch item; missing/foreign identities or
other owner errors are not converted into missing prices by a NotFound branch.

Ordinary public classification derives from `PortErrorKind`. The only code-specific rules
use closed Pricing-owned codes: exact missing Product above, and unsupported selected Product
capability (`pricing.product_identity_capability_unavailable` /
`pricing.product_write_capability_unavailable`). Arbitrary dependency codes and messages are
not public copy or control-flow selectors. Retryability is the owner's exact flag, including
Unavailable/Timeout; presentation must not turn permanent missing capability into a retry loop.

## Public envelopes

Transport responses use typed kind plus the closed capability exception below. `owner` means the exact owner retryable bit.

| Owner kind | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `PRICING_REQUEST_INVALID` | `Pricing query is invalid` | owner |
| NotFound | `PRICING_RESOURCE_NOT_FOUND` | `Pricing data was not found` | owner |
| Conflict | `PRICING_STATE_CONFLICT` | `Pricing state conflicts with this query` | owner |
| Forbidden | `PRICING_ACCESS_DENIED` | `Pricing query is not permitted` | owner |
| Unavailable / Timeout | `PRICING_TEMPORARILY_UNAVAILABLE` | `Pricing data is temporarily unavailable` | owner |
| InvariantViolation | `PRICING_OPERATION_FAILED` | `Pricing query could not be completed safely` | owner |
| Unavailable + closed unsupported capability code | `PRICING_CAPABILITY_UNAVAILABLE` | `The selected Product owner does not support this pricing operation` | owner (currently false) |

The complete owner `PortError`, owner message content and arbitrary owner codes are not copied into the GraphQL response. The retryable bit preserves the owner outcome.

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

- Pricing remains the canonical price/error owner; no transport-local resolver.
- Admin missing-Product and public optional-Product shapes remain intact.
- Unpriced batch items are distinct from identity/context/transport refusals.
- Successful DTOs carry owner source, normalized context and one resolution time.
- Pricing and Commerce FFA/FBA status is unchanged.
- No compile, SQL isolation, runtime, mounted GraphQL, remote or parity proof is claimed.

## Evidence

- `crates/modules/rustok-commerce/contracts/evidence/graphql-query-pricing-error-safety-source-review.json`
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

No tests, Cargo commands, mounted GraphQL scenarios, workflows or CI were executed.


## Nested owner classification continuation (2026-10-18)

Pricing now owns exhaustive nested Core/Rich classification; see the
[owner contract](../../rustok-pricing/docs/pricing-owner-port-error-safety.md#nested-error-and-public-consumer-continuation-2026-10-18).
This facade keeps query-specific static messages and existing optional-not-found behavior.
Remaining failures preserve `error.retryable` instead of suppressing it for domain kinds;
Unavailable/Timeout remain retryable even if an owner flag is false. Successful DTOs and
owner delegation are unchanged. The source guard now checks the actual `query_impl` module
and its scoped Pricing alias, not retired textual `include!` wiring. Standalone source
verification only; no tests, compilation, or runtime execution for this continuation.


## Pricing quote continuation (2026-10-29)

The resolver uses bounded batch quotes and an owner catalog projection. Five owner
calls replace the prior four-site/per-SKU projection flow: admin projection, active
lists, published catalog projection, public detail and batch quotes. Unpriced values
are explicit batch items; missing/foreign identities and transport errors no longer
turn into absent prices through a broad NotFound branch. Safe public classification
is still derived only from `PortErrorKind`, with the same static messages and bounded
owner-error shape. Product SKU UUID correlation and owner source/context/time are
retained; admin tenant input must match the current authenticated tenant.

Owner code strings and owner message text are not used for control flow. Pricing and
Commerce FFA/FBA status is unchanged. No tests, Cargo commands, mounted GraphQL scenarios,
workflows or CI were executed. The owner plan discloses scoped non-test checks.
