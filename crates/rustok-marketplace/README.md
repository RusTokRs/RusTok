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

`MarketplaceFinancialOrchestrationService` coordinates two idempotent owner stages:

1. assess order allocations through `MarketplaceCommissionCommandPort`;
2. post the resulting order batch through `MarketplaceLedgerCommandPort`.

The caller supplies one root idempotency key. The root derives stable child keys:

- `<root>:commission:v1`
- `<root>:ledger:v1`

The root stores no receipt. If the ledger stage is temporarily unavailable after
commission succeeds, retrying the same root command causes the commission owner to
replay its completed receipt and the ledger owner to retry its own stage. The root
also verifies that commission aggregates and ledger totals describe the same order
and amount.

Payout scheduling is intentionally not part of this workflow.

## Entry points

- `MarketplaceModule`
- `MarketplaceFamilyDescriptor`
- `MARKETPLACE_FAMILY_MODULES`
- `MarketplaceFinancialOrchestrationService`
- `MarketplaceFinancialCommandPort`
- owner-specific directory services

## Interactions

The root consumes owner projections and commands through FBA ports. Owner modules
remain independently deployable boundaries and publish their own module-owned FFA
packages. Host applications compose manifests and transports; they do not own
marketplace policy.
