# Marketplace family implementation plan

Last reviewed: 2026-07-16

## Status

- FFA status: `not_started`.
- FBA status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- Family source gate: `open`.
- Production promotion gate: `closed`.

## Ownership

- [x] Keep this module as the Marketplace Family root.
- [x] Own no seller, listing, commission, ledger, or payout tables.
- [x] Declare the required `rustok-marketplace-*` owner-module naming contract.
- [x] Publish a consumer registry for seller directory projections.
- [x] Compose `MarketplaceSellerReadPort` through the root-owned
  `MarketplaceSellerDirectoryService` without seller entities or database access.
- [ ] Add listing, allocation, commission, ledger, and payout consumers only after
  their owner contracts exist.

## FBA promotion

- [ ] Reach `boundary_ready` after the seller provider closes durable command
  identity, source contracts compile, and the central readiness board is safely
  synchronized.
- [ ] Retain remote-profile timeout, degraded-mode, and fallback evidence before
  `transport_verified`.

## FFA promotion

The root currently has no module-owned UI. Seller, listing, commission, ledger,
and payout UI must be published by the owning family modules. A future aggregate
Marketplace control room may be introduced only as a composition surface over
owner view models and transport facades.
