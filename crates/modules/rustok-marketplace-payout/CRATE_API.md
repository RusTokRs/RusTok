# rustok-marketplace-payout / CRATE_API

## Role

`rustok-marketplace-payout` is the canonical owner for marketplace seller payout scheduling and the assignment of seller-payable ledger entries to payout batches.
It groups payable sub-ledger entries into immutable payout batches for outbound bank and payment processor disbursement.

## Public surface

### Primary Public Types

- `MarketplacePayoutModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplacePayoutService`: Core domain service managing payout batches and scheduling.
- `MarketplacePayoutReadPort`, `MarketplacePayoutCommandPort`: Typed interface contracts for payout queries and scheduling.
- `MarketplacePayoutStatus`: Payout lifecycle status (`Scheduled`, `Submitted`, `Processing`, `Paid`, `Failed`, `Cancelled`).
- `ScheduleMarketplacePayoutInput`: Payload for scheduling a payout from selected ledger entries.
- `MarketplacePayoutResponse`, `MarketplacePayoutItemResponse`: Detailed payout record projections.
- `ReadMarketplacePayoutRequest`, `ListMarketplaceSellerPayoutsRequest`: Query requests.
- `MarketplacePayoutListResponse`: Paginated payout query response.
- `MAX_PAYOUT_ITEMS_PER_BATCH`, `MAX_PAYOUTS_PER_PAGE`: Bounded limit constants.
- `MarketplacePayoutError`, `MarketplacePayoutResult`: Domain error and result types.

## Platform Capability Bindings

- **RusToK Module Interface**: Implements `RusToKModule` with slug `"marketplace_payout"`, declaring dependencies on `"marketplace_balance"`, registering permissions for order reads and seller management.
- **Migration Source**: Implements `MigrationSource` registering database schema migrations for payout batches, item links, and idempotency receipts.

## Domain Collaborations

- **`rustok-marketplace-balance`**: Consumes `MarketplaceLedgerReadPort` to validate and claim seller-payable entries, and calls bucket transfer ports upon payout reservation.
- **`rustok-marketplace`**: Drives financial orchestration and payout tracking.

## Minimum Contract Set

### Input DTOs/Commands

- `ScheduleMarketplacePayoutInput`: `seller_id`, `currency`, `idempotency_key`, `ledger_entry_ids: Vec<Uuid>`.
- `ReadMarketplacePayoutRequest`: `payout_id`.
- `ListMarketplaceSellerPayoutsRequest`: `seller_id`, `currency`, `status`, `page`, `per_page`.

### Domain Invariants

- **Exclusive Entry Assignment**: A seller-payable ledger entry can belong to at most one payout batch; double-claiming is rejected.
- **Single Seller and Currency**: All ledger entries within a payout batch must share the identical tenant, seller, and currency.
- **Credit-Only Assignment**: Only credit entries in the `seller_payable` account with positive amounts can be included in a payout.
- **Atomic Batch Scheduling**: The payout header, line items, aggregate total amount, and completed receipt commit atomically in one database transaction.
- **Integer Minor Unit Precision**: All payout amounts are computed in checked integer minor units (`#![deny(clippy::float_arithmetic)]`).

### Events / Outbox Side Effects

- Atomically commits payout schedule headers and item assignments with command receipts in the same database transaction.

### Errors / Failure Codes

- `MarketplacePayoutError::NotFound`: Payout or referenced ledger entry does not exist.
- `MarketplacePayoutError::Conflict`: Ledger entry has already been claimed by another payout or idempotency conflict.
- `MarketplacePayoutError::Validation`: Currency mismatch across items, empty entry list, or batch size exceeds `MAX_PAYOUT_ITEMS_PER_BATCH`.
- `MarketplacePayoutError::Database`: Underlying persistence error wrapped cleanly.
