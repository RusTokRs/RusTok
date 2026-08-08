# Product lifecycle owner collaboration

Status: deployment co-requisite contract declared, control-plane enforcement pending, unvalidated.

## Problem

Product creation persists Product-owned variants and also accepts initial inventory and price input. The first Inventory and Pricing rows must commit or roll back with the Product write. Current source performs that collaboration through owner-owned, transaction-aware bootstrap services.

The ordinary module graph cannot represent the collaboration by adding reverse Product dependencies:

- Inventory depends on Product;
- Pricing depends on Product;
- making Product ordinarily depend on either owner would create a dependency-order cycle.

The default ecommerce deployment enables Product, Pricing, and Inventory together, but relying on that bundle implicitly leaves non-default deployment selection underspecified.

## Selected source contract

Product now declares Inventory and Pricing in `rustok-module.toml` under `[co_requisites]` with explicit version requirements. A co-requisite is a deployment-selection constraint, not an ordinary dependency edge:

- it says the owner implementation must be present in the selected deployment whenever Product lifecycle writes are available;
- it must not participate in dependency or migration ordering;
- it must not be copied into `ProductModule::dependencies()`;
- it therefore does not create `Product -> Inventory -> Product` or `Product -> Pricing -> Product` cycles.

This slice only declares and source-locks that contract. The current module control plane does not yet consume `[co_requisites]`; deployment enforcement and non-default selection evidence remain the next implementation step. Until that enforcement exists, the canonical ecommerce dependency-contract item must remain open.

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

`scripts/verify/verify-product-lifecycle-collaboration.mjs` source-locks that:

- Product's ordinary module dependency remains Taxonomy only;
- Product declares Inventory and Pricing as package co-requisites with bounded version requirements;
- Inventory and Pricing continue to depend ordinarily on Product;
- the default deployment bundle contains Product, Pricing, and Inventory;
- Product uses the exact owner bootstrap services and operations;
- Product does not import foreign owner entities or query known foreign tables directly;
- the owner services remain explicitly transaction-aware;
- control-plane co-requisite enforcement and standalone/non-default activation evidence remain unvalidated.

## Next implementation slice

Teach the static module control plane to parse and validate package co-requisites during deployment selection without feeding them into dependency ordering. The control plane must reject a selected Product module when an admitted Inventory or Pricing co-requisite is absent or version-incompatible, while preserving the existing ordinary reverse dependencies owned by Inventory and Pricing.

After source enforcement exists, retain maintainer-run evidence for:

- default and non-default module selections;
- clean migration ordering;
- Product create/delete rollback across all three owners;
- no silent post-commit partial state;
- no ordinary cyclic dependency;
- no Product ownership of Inventory or Pricing persistence.

Host-injected transaction participant ports remain a future alternative if Product lifecycle writes need a remote/distributed collaboration profile; they are not required to describe the current embedded atomic contract.

## Evidence

Source evidence is stored at:

`crates/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json`

No tests, Cargo commands, formatting, verifier execution, workflow checks, standalone activation, non-default deployment selection, or transaction rollback evidence are claimed by this source wave.
