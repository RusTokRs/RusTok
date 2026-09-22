# rustok-marketplace

## Purpose

`rustok-marketplace` is the umbrella/root module for the RusToK Marketplace Family.

## Responsibilities

- Declare the Marketplace Family and its owner modules.
- Compose typed marketplace provider/consumer ports.
- Coordinate cross-owner workflows without importing owner entities or creating
  duplicate persistence.
- Keep marketplace orchestration separate from general ecommerce orchestration in
  `rustok-commerce`.
- Own no seller, listing, allocation, commission, ledger, or payout tables.

## Family modules

- `rustok-marketplace-seller`
- `rustok-marketplace-listing`
- `rustok-marketplace-allocation`
- `rustok-marketplace-commission`
- `rustok-marketplace-ledger`
- `rustok-marketplace-payout`

## Financial orchestration

`MarketplaceFinancialOrchestrationService` coordinates owner commands with stable child
idempotency identities. The root owns no receipt or financial persistence.

### Initial order financials

The initial workflow has two idempotent owner stages:

1. assess order allocations through `MarketplaceCommissionCommandPort`;
2. post the resulting order batch through `MarketplaceLedgerCommandPort`.

The caller supplies one root idempotency key. The root derives:

- `<root>:commission:v1`
- `<root>:ledger:v1`

If the ledger stage is temporarily unavailable after commission succeeds, retrying the same root
command causes the commission owner to replay its completed receipt and the ledger owner to retry
its own stage. The root verifies that commission aggregates and ledger totals describe the same
order and amount.

### Refund and chargeback reversals

The reversal workflow accepts only normalized, typed financial facts. It does not consume raw
payment-provider payloads and does not reassess commission rules. The root derives:

- `<root>:ledger-reversal:v1`

It calls `MarketplaceLedgerCommandPort::post_financial_reversal` and validates the returned kind,
source, order, currency, source transaction, and balanced totals. The ledger owner preserves the
original posting and creates a new append-only correction transaction.

The payment/refund event listener that creates the normalized reversal request remains a separate
commerce-owned orchestration slice. The marketplace root does not parse provider metadata.

### Seller balance directory

`MarketplaceLedgerDirectoryService` exposes the ledger-owned seller balance projection through
`MarketplaceLedgerReadPort`. The projection is derived and rebuildable; the root never stores a
second balance.

Payout scheduling and payout settlement are intentionally not part of the initial financial or
reversal workflows.

## Entry points

- `MarketplaceModule`
- `MarketplaceFamilyDescriptor`
- `MARKETPLACE_FAMILY_MODULES`
- `MarketplaceFinancialOrchestrationService`
- `MarketplaceFinancialCommandPort`
- owner-specific directory services

## Interactions

The root consumes owner projections and commands through FBA ports. Owner modules remain
independently deployable boundaries and publish their own module-owned FFA packages. Host
applications compose manifests and transports; they do not own marketplace policy.
