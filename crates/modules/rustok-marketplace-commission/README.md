# rustok-marketplace-commission

## Purpose

`rustok-marketplace-commission` owns versioned commission rules and the immutable
commission assessment produced for each marketplace allocation.

## Boundary

- Allocation state is consumed only through `MarketplaceAllocationReadPort`.
- The module imports no allocation entities and declares no cross-module foreign keys.
- Rule versions are append-only. Updating a policy creates the next version for the
  same `rule_key`.
- An allocation receives at most one commission assessment.
- Assessment snapshots retain the exact rule id, rule key, version, rate, fixed
  amount, allocation total, commission, and seller proceeds.

## Rule selection

Eligible active rules are ordered deterministically:

1. listing-scoped rule;
2. seller-scoped rule;
3. global rule;
4. higher priority;
5. higher version;
6. later effective-from timestamp;
7. stable UUID tie-break.

A fixed component is eligible only when its currency matches the allocation.
Commission uses integer minor units:

`floor(allocation_total * rate_bps / 10000) + fixed_amount`

A rule result greater than the allocation total is rejected rather than producing
negative seller proceeds.

## Commands

- `create_rule_version` creates an immutable next rule version and durable receipt.
- `assess_order` replays before allocation provider reads, loads order allocations,
  then atomically inserts every assessment and completes its receipt.
- Partial order assessment is not allowed.

## Follow-on owners

Ledger posting, balance state, refund reversals, and payouts consume commission
assessments but remain separate modules.
