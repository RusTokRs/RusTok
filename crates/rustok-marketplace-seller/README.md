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
- Persist tenant-scoped seller command receipts with canonical SHA-256 request
  identity and normalized typed response snapshots.
- Commit a receipt, its seller/member mutation, and the saved response in one
  database transaction so a lost response can be replayed safely.
- Reject reuse of an idempotency key for another actor, command kind, or payload.
- Publish module-owned GraphQL query/mutation roots over the same typed ports.
- Publish a module-owned admin FFA package with explicit native/GraphQL transport
  selection and no implicit fallback.
- Store only normalized verification facts; provider-specific KYC payloads belong
  behind a future SPI and must not be persisted here.

## Entry points

- `MarketplaceSellerModule`
- `MarketplaceSellerService`
- `MarketplaceSellerReadPort`
- `MarketplaceSellerCommandPort`
- `graphql::MarketplaceSellerQuery` with the `graphql` feature
- `graphql::MarketplaceSellerMutation` with the `graphql` feature
- `dto::*`
- `entities::*`

## Interactions

The module is consumed by `rustok-marketplace`, module-owned admin transports, and
future listing/allocation modules through typed ports. The Leptos admin package
uses one serializable command envelope for both native server functions and
GraphQL, preserves the original idempotency key for explicit retry, and never
falls back to another transport automatically.

The module does not depend on `rustok-commerce`, copy auth users, own product
catalog content, or own payment, ledger, and payout state.
