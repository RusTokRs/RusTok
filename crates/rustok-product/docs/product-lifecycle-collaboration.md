# Product lifecycle owner collaboration

Status: contained source boundary, unresolved activation contract, unvalidated.

## Problem

Product creation persists Product-owned variants and also accepts initial inventory and price input. The first Inventory and Pricing rows must commit or roll back with the Product write. Current source performs that collaboration through owner-owned, transaction-aware bootstrap services.

The module graph cannot represent this by adding ordinary Product dependencies:

- Inventory depends on Product;
- Pricing depends on Product;
- making Product depend on either owner would create a dependency cycle.

The default ecommerce deployment currently enables Product, Pricing, and Inventory together, but that does not prove Product write behavior for every optional-module selection.

## Current contained boundary

Product owns:

- products, variants, options, translations, images, tags, and Product events;
- Product transaction admission and commit;
- collection of initial inventory and price requests from Product input.

Inventory owns:

- stock locations, inventory items, inventory levels, and reservation cleanup;
- `rustok_inventory::BootstrapService`;
- `InitialInventory` and the transaction-aware create/read/delete operations.

Pricing owns:

- price persistence and decimal/legacy amount normalization;
- `rustok_pricing_persistence::BootstrapService`;
- `InitialPrice` and the transaction-aware create/read/delete operations.

Product source imports only those owner bootstrap contracts. It does not import foreign Inventory/Pricing ORM entities and does not issue direct SQL against their tables.

## Atomicity

The owner bootstrap methods accept the same SeaORM transaction connection used by `ProductWriteTransaction`. Product variant state, initial inventory, initial prices, the Product event, and the outbox write therefore share one transaction boundary in the embedded profile.

This is a native transaction collaboration contract, not a GraphQL, REST, gRPC, or FBA transport contract. No remote write fallback is claimed.

## Guarded invariants

`scripts/verify/verify-product-lifecycle-collaboration.mjs` requires:

- Product's ordinary module dependency remains Taxonomy only;
- Inventory and Pricing continue to depend on Product;
- the default deployment bundle contains Product, Pricing, and Inventory;
- Product uses the exact owner bootstrap services and operations;
- Product does not import foreign owner entities or query known foreign tables directly;
- the owner services remain explicitly transaction-aware;
- evidence remains unvalidated and does not claim standalone activation.

## Open architecture decision

Choose one design before calling the dependency contract resolved:

1. add a control-plane co-requisite concept that affects module selection without creating dependency-order cycles; or
2. publish host-injected transaction participant ports through a neutral contract crate and compose embedded Inventory/Pricing adapters without changing atomicity.

The selected design must prove:

- Product write startup and activation for non-default module selections;
- clean migration ordering;
- create/delete rollback across all three owners;
- no silent post-commit partial state;
- no ordinary cyclic dependency;
- no Product ownership of Inventory or Pricing persistence.

## Evidence

Source evidence is stored at:

`crates/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json`

No tests, Cargo commands, formatting, verifier execution, workflow checks, standalone activation, or transaction rollback evidence are claimed by this source wave.
