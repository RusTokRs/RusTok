# rustok-marketplace / CRATE_API

## Role

`rustok-marketplace` is the root and cross-module orchestration boundary for the RusToK Marketplace Family.
It coordinates multi-seller order workflows, initial financial allocations, commission assessments, ledger postings, and refund/chargeback reversals without owning separate tables or duplicating domain state.

## Public surface

### Primary Public Types

- `MarketplaceModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplaceFamilyDescriptor`: Family metadata struct with `root_slug` and `owner_modules`.
- `MARKETPLACE_FAMILY_MODULES`: Canonical list of member module slugs in the Marketplace Family.
- `MarketplaceFinancialOrchestrationService`: Orchestrator coordinating commission assessment and ledger postings.
- `MarketplaceFinancialCommandPort`: Port trait defining cross-module financial operations.
- `ProcessMarketplaceOrderFinancialsInput`, `ProcessMarketplaceOrderFinancialsResponse`: Initial order financial DTOs.
- `ProcessMarketplaceFinancialReversalInput`, `ProcessMarketplaceFinancialReversalResponse`: Financial reversal DTOs.
- `MarketplaceFinancialOrchestrationError`, `MarketplaceFinancialOrchestrationResult`: Error and result types.
- `MarketplaceSellerDirectoryService`: Directory adapter for seller discovery.
- `MarketplaceListingDirectoryService`: Directory adapter for listing catalog projections.
- `MarketplaceAllocationDirectoryService`: Directory adapter for item allocations.
- `MarketplaceCommissionDirectoryService`: Directory adapter for commission rates.
- `MarketplaceLedgerDirectoryService`: Directory adapter for seller balance and ledger projections.
- `MarketplacePayoutDirectoryService`: Directory adapter for payout tracking.

## Platform Capability Bindings

- **RusToK Module Interface**: Implements `RusToKModule` with slug `"marketplace"`, declaring dependencies on all Marketplace Family modules (`marketplace_seller`, `marketplace_listing`, `marketplace_allocation`, `marketplace_commission`, `marketplace_balance`, `marketplace_payout`).
- **Migration Source**: Implements `MigrationSource` returning empty migrations (root owns no dedicated persistence schema).
- **FBA Port Composition**: Bridges domain ports between child marketplace modules and the host application.

## Domain Collaborations

- **`rustok-marketplace-seller`**: Consumes seller ports for identity and status verification.
- **`rustok-marketplace-listing`**: Consumes listing ports for commercial eligibility and terms.
- **`rustok-marketplace-allocation`**: Consumes order allocation splits across sellers.
- **`rustok-marketplace-commission`**: Invokes commission assessments on multi-seller order lines.
- **`rustok-marketplace-balance`**: Dispatches double-entry ledger postings and reversals.
- **`rustok-marketplace-payout`**: Bridges seller payout requests to payout engines.

## Minimum Contract Set

### Input DTOs/Commands

- `ProcessMarketplaceOrderFinancialsInput`: `order_id`, `currency`, `allocations`, `idempotency_key`.
- `ProcessMarketplaceFinancialReversalInput`: `order_id`, `reversal_kind`, `amount`, `currency`, `idempotency_key`.

### Domain Invariants

- **Zero Dedicated Persistence**: The root module must not declare database tables or own entity state; all facts are owned by child crates.
- **Deterministic Child Idempotency**: Derived idempotency keys follow stable prefixes (`<root>:commission:v1`, `<root>:ledger:v1`, `<root>:ledger-reversal:v1`).
- **Balanced Totals Verification**: Commission assessments and ledger entries must reconcile to identical order totals before completion.
- **Append-Only Reversals**: Reversals preserve the original postings and append compensating ledger transactions.

### Events / Outbox Side Effects

- Emits no root-specific events directly; coordinates child modules which emit their respective transactional outbox events atomically.

### Errors / Failure Codes

- `MarketplaceFinancialOrchestrationError::CommissionFailed`: Commission assessment stage failed.
- `MarketplaceFinancialOrchestrationError::LedgerFailed`: Posting to ledger failed.
- `MarketplaceFinancialOrchestrationError::AmountMismatch`: Reconciled totals between stages do not match.
- `MarketplaceFinancialOrchestrationError::InvalidInput`: Input payload failed schema or business validation constraints.
- `MarketplaceFinancialOrchestrationError::PortError`: Port invocation failed across module boundary.
