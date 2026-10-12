# `rustok-marketplace-ledger` Implementation Plan

## Scope

`rustok-marketplace-ledger` owns seller escrow balances, wallet bucket transitions (`pending` -> `available` -> `reserved` -> `paid`), commission assessments, and payout reservation accounting for marketplace sellers.

## Current state

Part of the Marketplace Family under `crates/modules/rustok-commerce/docs/implementation-plan.md`.
Status: `in_progress`.

## Architectural Alignment with `rustok-accounting`

1. **Role Clarification (Seller Escrow Wallet):**
   `rustok-marketplace-ledger` is retained as the dedicated domain owner for seller escrow balances and payout lifecycle states (`pending`, `available`, `reserved`, `paid`). It is not the general accounting ledger for the platform.
2. **FBA Integration Seam:**
   Universal double-entry bookkeeping, Chart of Accounts, and tax/statutory ledgers belong to `rustok-accounting`.
   `rustok-marketplace-ledger` connects to `rustok-accounting` through typed FBA port contracts and `TransactionalEventBus`, translating order settlements, commission deductions, and payout transfers into platform-wide G/L journal entries without polluting the core accounting domain with seller-specific bucket schemas.

## Milestones

1. Domain core & command receipt ledger (seller balance buckets);
2. FBA port contracts & GraphQL transport;
3. Integration seam with `rustok-accounting` (FBA port adapter for platform G/L journal postings);
4. Admin & storefront integration.

## Verification

- `cargo test -p rustok-marketplace-ledger`
- `cargo xtask module validate marketplace_ledger`

## Change rules

Changes must align with the unified Marketplace Family architecture in `crates/modules/rustok-commerce/docs/implementation-plan.md` and the platform accounting architecture in `docs/architecture/crm-erp-accounting-matrix-and-plan.md`.
