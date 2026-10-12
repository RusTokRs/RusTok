# `rustok-marketplace-balance` Documentation

## Purpose

`rustok-marketplace-balance` owns seller balances, escrow holds, and financial bucket transitions within the RusToK Marketplace family.

## Scope

- Domain models and storage for seller balances, escrow allocations, and payout reservations;
- Seller balance bucket lifecycle: `pending` -> `available` -> `reserved` -> `paid`;
- Idempotent command ports and typed balance query boundaries;
- FBA bridge to `rustok-accounting` for statutory general ledger postings.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Integrates with `rustok-marketplace-commission` for assessment deductions;
- Integrates with `rustok-marketplace-payout` for payout reservations;
- Integrates with `rustok-accounting` via `MarketplaceAccountingBridge`.

## Verification

- `cargo check -p rustok-marketplace-balance`
- `cargo test -p rustok-marketplace-balance`
- `cargo xtask module validate marketplace_balance`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
