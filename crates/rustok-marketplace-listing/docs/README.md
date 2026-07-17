# Marketplace listing runtime contract

`rustok-marketplace-listing` is the listing owner module of the Marketplace
Family. The listing aggregate owns identity, lifecycle, approval, current terms
version, and publication state. `marketplace_listing_terms` stores immutable
commercial-reference versions.

Seller and product references are validated through `MarketplaceSellerReadPort`
and `ProductCatalogReadPort`. The listing schema does not use cross-module foreign
keys and does not copy canonical product content, prices, stock, or fulfillment
state.

Read ports require deadline semantics. Command ports require deadlines and stable
idempotency keys. A tenant-scoped command receipt binds actor, command kind, and
canonical SHA-256 request identity; receipt, owner mutation, and the normalized
listing response commit in the same transaction.

Eligibility is a read projection with stable reason codes. It checks listing
lifecycle, approval, publication, commercial references, and current seller
status. It intentionally does not implement buy-box ranking.

The canonical family roadmap is maintained in
`crates/rustok-commerce/docs/implementation-plan.md`.
