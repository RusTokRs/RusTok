# Marketplace module runtime contract

`rustok-marketplace` is the root of the Marketplace Family.

It composes typed owner ports and future cross-marketplace workflows. It must not
own seller, listing, commission, ledger, or payout persistence. General ecommerce
checkout and post-order orchestration remain in `rustok-commerce`; marketplace
specific allocation and settlement workflows enter through explicit typed ports.

The canonical family roadmap is maintained in
`crates/rustok-commerce/docs/implementation-plan.md`.
