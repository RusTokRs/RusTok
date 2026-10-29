# rustok-marketplace-balance / CRATE_API

## Role

`rustok-marketplace-balance` is the canonical owner for marketplace seller balances, escrow holds, double-entry sub-ledger postings, and financial bucket transitions.
It records balanced debit/credit transactions for marketplace orders, append-only reversals (refunds/chargebacks), and escrow transfers between pending, available, and reserved buckets.

## Public surface

### Primary Public Types

- `MarketplaceBalanceModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplaceLedgerService`: Core domain service managing ledger transactions, entries, balance rebuilds, and receipts.
- `MarketplaceLedgerReadPort`, `MarketplaceLedgerCommandPort`: Typed interface contracts for ledger queries and postings.
- `MarketplaceLedgerAccountCode`: System ledger account codes (`MarketplaceClearing`, `CommissionRevenue`, `SellerPayable`).
- `MarketplaceLedgerEntryDirection`: Posting direction (`Debit`, `Credit`).
- `MarketplaceSellerBalanceBucket`: Balance sub-bucket (`Pending`, `Available`, `Reserved`, `Paid`).
- `MarketplaceLedgerReversalKind`: Financial reversal source (`Refund`, `Chargeback`).
- `MarketplaceSellerBalanceTransferKind`: Bucket transition category (`ReleaseEscrow`, `ReservePayout`, `CancelPayoutReserve`, `SettlePayout`).
- `PostMarketplaceOrderLedgerInput`: Initial order ledger posting command.
- `PostMarketplaceLedgerReversalInput`: Refund/chargeback posting command.
- `PostMarketplaceSellerBalanceTransferInput`: Balance bucket transfer command.
- `MarketplaceSellerBalanceResponse`, `MarketplaceSellerPayableBalanceResponse`: Materialized seller balance projections.
- `MarketplaceLedgerTransactionResponse`, `MarketplaceLedgerEntryResponse`: Detailed transaction responses.
- `MarketplaceAccountingBridge`: Bridge mapping marketplace ledger records into platform general ledger journal inputs.
- `MarketplaceLedgerError`, `MarketplaceLedgerResult`: Domain error and result types.

## Platform Capability Bindings

- **RusToK Module Interface**: Implements `RusToKModule` with slug `"marketplace_balance"`, declaring dependencies on `"marketplace_commission"` and `"accounting"`, registering permissions for order reads and seller lookups.
- **Migration Source**: Implements `MigrationSource` registering database schema migrations for ledger transactions, entries, reversal records, balance projections, and receipts.
- **Accounting Bridge**: Exposes `MarketplaceAccountingBridge` for general ledger integration via `rustok-accounting`.

## Domain Collaborations

- **`rustok-marketplace-commission`**: Consumes commission assessment batches to post initial order ledger entries.
- **`rustok-accounting`**: Bridges sub-ledger double-entry postings to platform general ledger accounts.
- **`rustok-marketplace-payout`**: Coordinates payout reservations and settlements through bucket transfer ports.
- **`rustok-marketplace`**: Drives financial orchestration and reversal workflows across orders.

## Minimum Contract Set

### Input DTOs/Commands

- `PostMarketplaceOrderLedgerInput`: `order_id`, `idempotency_key`.
- `PostMarketplaceLedgerReversalInput`: `order_id`, `reversal_kind`, `source_id`, `idempotency_key`, `lines: Vec<MarketplaceLedgerReversalLineInput>`.
- `PostMarketplaceSellerBalanceTransferInput`: `seller_id`, `transfer_kind`, `source_id`, `currency`, `idempotency_key`, `lines: Vec<MarketplaceSellerBalanceTransferLineInput>`.
- `ReadMarketplaceSellerBalanceRequest`: `seller_id`, `currency`.
- `RebuildMarketplaceSellerBalanceInput`: `seller_id`, `currency`.

### Domain Invariants

- **Double-Entry Balance**: Total debits must strictly equal total credits for every committed transaction (`sum(debits) == sum(credits)`).
- **Append-Only Postings**: Committed ledger transactions and entries are immutable; corrections and reversals create new balanced transactions.
- **Checked Minor Unit Arithmetic**: All amounts are signed 64-bit integers in minor currency units; float arithmetic is prohibited (`#![deny(clippy::float_arithmetic)]`).
- **Rebuildable Projections**: Seller balance projections are derived and can be completely reconstructed from authoritative ledger entries via `rebuild_seller_balance`.
- **Cumulative Capacity Limits**: Reversals cannot exceed original line entries, and bucket transfers cannot exceed available source bucket capacity.

### Events / Outbox Side Effects

- Atomically commits ledger transactions with idempotency receipts; balance rebuilds update materialized seller balance records in the same transaction.

### Errors / Failure Codes

- `MarketplaceLedgerError::NotFound`: Transaction, entry, or commission batch does not exist.
- `MarketplaceLedgerError::Unbalanced`: Transaction debits do not equal credits.
- `MarketplaceLedgerError::CapacityExceeded`: Reversal amount or transfer amount exceeds original entry limit.
- `MarketplaceLedgerError::Conflict`: Duplicate posting for commission order or conflicting idempotency key.
- `MarketplaceLedgerError::Database`: Underlying persistence error wrapped cleanly.
