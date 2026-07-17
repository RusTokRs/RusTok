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
- [x] Publish consumer registries for seller and listing directory projections.
- [x] Compose `MarketplaceSellerReadPort` through a root-owned directory service.
- [x] Compose `MarketplaceListingReadPort` through a root-owned directory and
  eligibility service without owner entity/database imports.
- [ ] Add allocation, commission, ledger, and payout consumers only after their
  owner contracts exist.

## FBA promotion

- [ ] Reach `boundary_ready` after typed seller/listing provider-consumer ports,
  in-process providers, deadline/error rules, durable command identity, source
  guards, and compiled contract evidence are aligned.
- [ ] Retain remote-profile timeout, degraded-mode, and fallback evidence before
  `transport_verified`.

## FFA promotion

The root currently has no module-owned UI. Seller, listing, commission, ledger,
and payout UI must be published by the owning family modules. A future aggregate
Marketplace control room may be introduced only as a composition surface over
owner view models and transport facades.

## Source evidence

- `src/seller_directory.rs`
- `src/listing_directory.rs`
- `contracts/marketplace-fba-registry.json`
- `../rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json`
- `../rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json`
- `../../apps/server/tests/marketplace_family_boundary_guard.rs`
- `../../apps/server/tests/marketplace_listing_boundary_guard.rs`
- `../../scripts/verify/verify-marketplace-family-boundary.mjs`
- `../../scripts/verify/verify-marketplace-listing-boundary.mjs`
