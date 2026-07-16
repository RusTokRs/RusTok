# rustok-marketplace-seller

## Purpose

`rustok-marketplace-seller` owns seller identity, seller lifecycle, onboarding,
and seller-scoped memberships for the RusToK Marketplace Family.

## Responsibilities

- Persist tenant-scoped seller identity and profile data.
- Own seller lifecycle transitions: draft, active, suspended, and closed.
- Own onboarding review state independently from business lifecycle state.
- Create the initial owner membership atomically with a seller.
- Own seller-scoped member roles and status without replacing platform RBAC.
- Publish typed FBA read and command ports.
- Publish a module-owned admin FFA package.
- Store only normalized verification facts; provider-specific KYC payloads belong
  behind a future SPI and must not be persisted here.

## Entry points

- `MarketplaceSellerModule`
- `MarketplaceSellerService`
- `MarketplaceSellerReadPort`
- `MarketplaceSellerCommandPort`
- `dto::*`
- `entities::*`

## Interactions

The module is consumed by `rustok-marketplace`, module-owned admin transports, and
future listing/allocation modules through typed ports. It does not depend on
`rustok-commerce`, copy auth users, own product catalog content, or own payment,
ledger, and payout state.
