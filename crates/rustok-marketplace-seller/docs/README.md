# Marketplace seller runtime contract

`rustok-marketplace-seller` is the seller owner module of the Marketplace Family.

Persistence and lifecycle commands remain owner-owned. Consumers use
`MarketplaceSellerReadPort` and `MarketplaceSellerCommandPort` with
`rustok_api::ports::PortContext` and `PortError`. Read calls require deadlines;
write calls require deadlines and stable idempotency keys.

Platform permissions use the `marketplace_sellers` resource. Seller membership
roles remain seller-owned and do not create a second platform RBAC system.

The canonical family roadmap is maintained in
`crates/rustok-commerce/docs/implementation-plan.md`.
