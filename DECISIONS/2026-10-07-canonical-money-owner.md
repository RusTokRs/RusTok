# Currency exponents, minor units, and money rounding are owned by `rustok-core::money`

- Date: 2026-10-07
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-core` (platform foundation) owns the money primitives; `rustok-cart`, `rustok-order`,
  `rustok-pricing`, `rustok-pricing-persistence`, `rustok-commerce`, and `rustok-payment` are consumers
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Every money path needs the same three rules: the minor-unit exponent of a currency, currency-aware
rounding, and major-unit <-> minor-unit conversion. Before this decision the platform had four
divergent copies of the exponent table (`rustok-cart::services::cart::helpers`,
`rustok-order::services::order`, `rustok-pricing::services::pricing`,
`rustok-payment::stripe_provider`) plus fixed two-decimal conversions in `rustok-pricing` and
`rustok-pricing-persistence`. The e-commerce deep review (`docs/audits/ecommerce-deep-review-2026-10-07.md`,
ECOM-MONEY-02/03/04) found that the copies disagreed for ISK, MGA, IQD, and LYD, that conversion
failures were silently replaced by `0` or by a stale value, and that the amount that reaches Stripe
is produced by a fifth private table.

Two constraints make this an ownership decision, not a cleanup:

1. `AGENTS.md` §5 requires common platform kernel primitives — explicitly including money/currency
   types — to live in canonical platform foundation libraries (`crates/libs/*`) without cross-module
   duplication. `rustok-commerce-foundation` lives in `crates/modules/*` and is a module crate, so it
   cannot be the platform owner even though it is the declared shared commerce foundation.
2. A payment provider legitimately defines its own minor-unit contract for some currencies. The
   platform cannot simply mandate one table for a PSP adapter; it must give the adapter a way to
   implement the PSP contract explicitly instead of keeping a private copy of the platform table.

## Decision

`rustok-core::money` is the single owner of currency rules for the platform:

- the ISO 4217 exponent table (`ZERO_DECIMAL_CURRENCIES`, `THREE_DECIMAL_CURRENCIES`,
  `FOUR_DECIMAL_CURRENCIES`, `UNDEFINED_MINOR_UNIT_CURRENCIES`),
- currency-code normalization (`normalize_currency_code`) and exponent lookup (`currency_exponent`),
- currency-aware rounding (`round_to_currency`, `round_to_fixed`),
- major-unit <-> minor-unit conversion (`to_minor_units`, `to_minor_units_exact`,
  `to_fixed_point_units`, `to_fixed_point_units_exact`, `from_minor_units`,
  `from_fixed_point_units`), all failing with the typed `MoneyError`.

Rules that follow from the decision:

- Consumers MUST NOT keep a private exponent table, a private `round_dp(2)` shortcut for money, or a
  conversion that returns `Option`/`0` on failure. They map `MoneyError` into their own error type
  and preserve the reason.
- Payment adapters MUST use the exponent-explicit primitives with their own documented, tested
  per-currency override table. `rustok-payment` keeps `STRIPE_EXPONENT_DIVERGENCES` (MGA = 0,
  ISK = 2, IQD = 2, LYD = 2, per Stripe's published amount contract) and
  `STRIPE_WHOLE_MAJOR_UNIT_CURRENCIES` (ISK, which Stripe documents as unchargeable in fractions).
- The retained two-decimal pricing mirror columns stay two-decimal: `rustok-pricing-persistence`
  names that precision (`LEGACY_AMOUNT_EXPONENT`) and performs the conversion through the owner.

This supersedes the provisional recommendation in the deep-review report, which suggested
`rustok-commerce-foundation::money`: that crate is a module crate, and `rustok-order`/`rustok-payment`
would have needed a new module-to-module dependency edge. `rustok-core` is already a dependency of
every money-touching crate, so the decision adds no new edge.

## Sources of truth and ownership

- Canonical owner: `rustok-core::money` (source file `crates/libs/rustok-core/src/money.rs`).
- Authoritative persisted state: the currency code and amount columns owned by each module; stored
  `currency_exponent` columns remain owner-local data validated by their own persistence guards
  (`0..=9`) and are consumed through the exponent-explicit API.
- Derived data: the two-decimal `legacy_amount` / `legacy_compare_at_amount` pricing mirror columns
  are derived, not authoritative; they keep their historical precision.
- Dependency direction: `rustok-core` gains `rust_decimal` and stays free of module dependencies;
  commerce-family modules depend on `rustok-core` as before.

## Invariants

### Allowed states

- A conversion either returns an exact, in-range value or a typed `MoneyError`.
- A well-formed code that is not listed in the tables maps to the ISO default of two decimals.
- A PSP adapter returns the exponent that its provider documents for a currency.

### Forbidden states

- A silent `0` (or any other placeholder) on conversion failure.
- A silent two-decimal truncation of a currency whose minor unit is not 1/100.
- A conversion in runtime code that panics on a malformed currency code or an unrepresentable amount.
- A private exponent table outside the owner, or a PSP adapter that inherits the platform table
  without documenting a divergence.

## Non-goals

- Foreign-exchange conversion, rate sourcing, and settlement-currency decisions.
- Tax rounding policy and invoice presentation rules.
- Per-tenant synthetic currencies with more than four decimals; the platform accepts well-formed
  three-letter codes only.
- Currency metadata beyond the exponent (symbols, locale formatting, minimum charge amounts).

## Data, transaction, and concurrency boundary

- No persisted state is introduced and no migration is required. Existing database guards on stored
  exponents (`0..=9`) remain in force and are consistent with `MAX_MINOR_UNIT_EXPONENT` (9).
- Conversions are pure functions; they never open transactions and never read the database.
- The `legacy_amount` mirror columns keep writing the two-decimal value (or `NULL` when the amount is
  unrepresentable in two decimals), exactly as before.

## Context dimensions

- Currency: this decision's subject; the exponent is derived from the normalized currency code only.
- Tenant/channel/locale/principal: not applicable — no money rule depends on them.
- Time/timezone: not applicable.

## Events and projections

Domain events that carry minor units (`order.placed`, `price.updated`) convert through
`rustok_core::money` at publish time, so the event scale is the currency's scale and
invalidation/projection consumers can rely on it. No event shape changes: the payloads already carry
minor units plus the currency code.

## Failure semantics

- Malformed codes, non-decimal ISO codes (metals, SDR, testing codes) and unsupported exponents fail
  with a typed error — fail closed, never a guessed scale.
- Providers whose documented contract differs from the platform table are handled by the adapter's
  explicit divergence list, so a platform-table change cannot silently change a PSP amount.
- Fractional amounts for currencies the PSP cannot charge are rejected before the HTTP call.

## Migration and cutover

- Zero-legacy cutover in one change set: the four private tables/functions and the fixed two-decimal
  helpers were deleted in the same change that introduced the owner; no compatibility wrapper remains.
- No data migration: stored amounts and stored exponents are unchanged, because the owner reproduces
  the platform table exactly (including the corrected divergence list).
- External compatibility: the Stripe mapping is documented and covered by per-currency tests rather
  than changed silently; the two-decimal pricing mirror columns are explicitly non-authoritative.

## Alternatives considered

- `rustok-commerce-foundation::money` (the review's provisional recommendation): rejected because
  `AGENTS.md` §5 requires kernel primitives in `crates/libs/*`, and `rustok-order`/`rustok-payment`
  would have to add a module-to-module dependency.
- Keep the copies and add a synchronization gate: rejected — a gate detects drift only after the
  fact, and the copies had already diverged in production-relevant ways.
- Extend `rustok-api` or `rustok-events` with money helpers: rejected as a responsibility mismatch;
  `rustok-core` is the platform kernel library.
- Keep an `Option`-returning conversion for the legacy mirror columns without a typed error: rejected
  for the money path (silent failures), retained only for the documented nullable mirror column where
  `NULL` is the column's own representation.

## Verification

- Unit tests in `crates/libs/rustok-core/src/money.rs`: exponent table (including the corrected MGA
  and undefined-minor-unit cases), currency-aware rounding, exact conversion rejection of excess
  precision, out-of-range behaviour, and unsupported exponents.
- Unit tests in `crates/modules/rustok-payment/src/stripe_provider.rs`: each Stripe divergence, the
  platform parity set, fractional-ISK rejection, and undefined/malformed currency rejection.
- Repository verification: the 213-verifier sweep reports the same verdicts before and after the
  change (98 PASS / 115 FAIL), plus `verify-docs` and `verify-adrs` for this document.

## Consequences

- Money correctness is centralized: a new currency or provider change is one reviewed edit plus
  tests, not a hunt for copies.
- `rustok-core` gains a `rust_decimal` dependency; it is already in the workspace and used by every
  consumer.
- Money paths no longer mask conversion failures: callers that previously fell back to `0` or to a
  stale value now fail with a typed error, which is the intended behaviour for money.
- Future providers must add their own divergence table through the exponent-explicit API; the review
  checklist for new PSP adapters must include one test per divergent currency.
