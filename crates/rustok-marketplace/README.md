# rustok-marketplace

## Purpose

`rustok-marketplace` is the umbrella/root module for the RusToK Marketplace Family.

## Responsibilities

- Declare the Marketplace Family and its owner modules.
- Compose typed marketplace provider/consumer ports.
- Own future cross-marketplace workflows such as seller allocation coordination.
- Keep marketplace orchestration separate from general ecommerce orchestration in
  `rustok-commerce`.
- Own no seller, listing, commission, ledger, or payout tables.

## Family modules

- `rustok-marketplace-seller`
- `rustok-marketplace-listing`
- `rustok-marketplace-commission`
- `rustok-marketplace-ledger`
- `rustok-marketplace-payout`

## Entry points

- `MarketplaceModule`
- `MarketplaceFamilyDescriptor`
- `MARKETPLACE_FAMILY_MODULES`

## Interactions

The root consumes owner projections through FBA ports. Owner modules remain
independently deployable boundaries and publish their own module-owned FFA
packages. Host applications compose manifests and transports; they do not own
marketplace policy.
