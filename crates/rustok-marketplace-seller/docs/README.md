# Marketplace seller runtime contract

`rustok-marketplace-seller` is the seller owner module of the Marketplace Family.

Persistence and lifecycle commands remain owner-owned. Consumers use
`MarketplaceSellerReadPort` and `MarketplaceSellerCommandPort` with
`rustok_api::ports::PortContext` and `PortError`. Read calls require deadlines;
write calls require deadlines and stable idempotency keys.

Every command-port write is admitted through
`marketplace_seller_command_receipts`. The immutable receipt identity is scoped by
tenant and idempotency key and binds the actor, command kind, and normalized
canonical SHA-256 request hash. The receipt, seller/member mutation, and normalized
typed response snapshot commit in the same transaction. A repeated identical
command returns the saved response; a different command or payload returns the
stable `marketplace_seller.idempotency_conflict` error.

The optional module-owned GraphQL roots and the admin native/GraphQL adapters call
the same typed ports. The admin package selects exactly one transport through
`execute_selected_transport`; it does not automatically fall back. Explicit retry
reuses the original idempotency key and command envelope.

Platform permissions use the `marketplace_sellers` resource. Seller membership
roles remain seller-owned and do not create a second platform RBAC system.
Database and driver details are not exposed through GraphQL, server functions, or
FBA errors.

The canonical family roadmap is maintained in
`crates/rustok-commerce/docs/implementation-plan.md`.
