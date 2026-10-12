# `rustok-marketplace-balance` Implementation Plan

## Scope

`rustok-marketplace-balance` owns seller escrow balances, balance bucket transitions (`pending` -> `available` -> `reserved` -> `paid`), commission deductions, and payout reservation tracking for marketplace sellers.

## Current state

Part of the Marketplace Family under `crates/modules/rustok-commerce/docs/implementation-plan.md`.
Status: `in_progress`.
FFA status: `not_started`.
FBA status: `boundary_ready`.

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `marketplace_ledger_entries`, `balances`
- Native migrations: `m20260721_000001_create_marketplace_ledger`, `m20260721_000002_add_receipt_replay_receipts`, `m20260721_000003_add_seller_balance_transfers`
- Supported migration policy: AdditiveOnly
- Predecessor standby strategy: Standby DB + Slot
- Rollback eligibility: DeniedMaintenanceOnly
- Recovery invariants: Financial records immutable, append-only double-entry balance buckets

## Architectural Alignment with `rustok-accounting`

1. **Role Clarification (Seller Escrow Balance):**
   `rustok-marketplace-balance` is retained as the dedicated domain owner for seller escrow balances and payout lifecycle states (`pending`, `available`, `reserved`, `paid`). It is not the general accounting ledger for the platform.
2. **FBA Integration Seam:**
   Universal double-entry bookkeeping, Chart of Accounts, and tax/statutory ledgers belong to `rustok-accounting`.
   `rustok-marketplace-balance` connects to `rustok-accounting` through typed FBA port contracts and `TransactionalEventBus`, translating order settlements, commission deductions, and payout transfers into platform-wide G/L journal entries without polluting the core accounting domain with seller-specific bucket schemas.

## Milestones

1. Domain core & command receipt ledger (seller balance buckets);
2. FBA port contracts & GraphQL transport;
3. Integration seam with `rustok-accounting` (`MarketplaceAccountingBridge` for platform G/L journal postings);
4. Admin & storefront integration.

## Verification

- `cargo check -p rustok-marketplace-balance`
- `cargo test -p rustok-marketplace-balance`
- `cargo xtask module validate marketplace_balance`

## Change rules

Changes must align with the unified Marketplace Family architecture in `crates/modules/rustok-commerce/docs/implementation-plan.md` and the platform accounting architecture in `docs/architecture/crm-erp-accounting-matrix-and-plan.md`.
