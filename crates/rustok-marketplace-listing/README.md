# rustok-marketplace-listing

## Purpose

`rustok-marketplace-listing` owns seller listing identity, versioned commercial
terms, lifecycle, approval, and deterministic eligibility projections for the
RusToK Marketplace Family.

## Responsibilities

- Keep one listing identity per tenant, seller, master variant, market, and
  channel scope.
- Keep seller SKU uniqueness inside seller scope.
- Store commercial references in immutable listing-term versions instead of
  copying product, pricing, inventory, or fulfillment owner data.
- Resolve seller and master variant references only through typed owner ports.
- Own draft, review, publish, suspend, reactivate, and archive transitions.
- Persist command receipts atomically with owner writes, immutable internal
  timeline events, sealed external contract events, and typed response snapshots.
- Publish external listing events through the transactional outbox without
  exposing moderation prose, arbitrary owner metadata, or imported legacy snapshots.
- Publish deterministic eligibility with explicit reason codes.
- Avoid buy-box ranking; selection policy belongs to a later Marketplace Family
  capability.

## Entry points

- `MarketplaceListingModule`
- `MarketplaceListingService`
- `MarketplaceListingReadPort`
- `MarketplaceListingCommandPort`
- `dto::*`
- `entities::*`

## Interactions

The module consumes `MarketplaceSellerReadPort` and `ProductCatalogReadPort`.
It owns no seller or product entities and declares no cross-module database
foreign keys. `rustok-marketplace` consumes its read projections through a typed
port. External consumers receive sealed listing lifecycle contracts through
`TransactionalEventBus` and refresh owner state through `MarketplaceListingReadPort`.
