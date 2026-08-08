# Product lifecycle owner collaboration

Status: static deployment co-requisite enforcement source-complete, tenant lifecycle enforcement pending, unvalidated.

## Problem

Product creation persists Product-owned variants and also accepts initial inventory and price input. The first Inventory and Pricing rows must commit or roll back with the Product write. Current source performs that collaboration through owner-owned, transaction-aware bootstrap services.

The ordinary module graph cannot represent the collaboration by adding reverse Product dependencies:

- Inventory depends on Product;
- Pricing depends on Product;
- making Product ordinarily depend on either owner would create a dependency-order cycle.

The default ecommerce activation enables Product, Pricing, and Inventory together. Source must still reject a deployment-default selection that enables Product without those owner implementations while preserving acyclic installation and migration ordering.

## Selected source contract

Product declares Inventory and Pricing in `rustok-module.toml` under `[co_requisites]` with explicit version requirements. A co-requisite is a deployment-selection constraint, not an ordinary dependency edge:

- it says the owner implementation must be selected whenever Product lifecycle writes are available;
- it must not participate in dependency or migration ordering;
- it must not be copied into `ProductModule::dependencies()`;
- it therefore does not create `Product -> Inventory -> Product` or `Product -> Pricing -> Product` cycles.

The static server control plane now consumes this declaration through `ManifestManager::validate_deployment_selection`. Startup `validate_registry_vs_manifest` first runs the existing ordinary manifest validation, then the deployment co-requisite preflight, then the ordinary static-registry comparison. The preflight reads selected package `[co_requisites]` and rejects a default-enabled Product when Inventory or Pricing is not also selected, when a declared requirement is malformed, or when the selected owner version is missing, invalid, or incompatible.

Deployment co-requisite validation is intentionally separate from `ManifestManager::validate`. Built-in installation and ordinary topology validation therefore remain able to add Product, Inventory, and Pricing sequentially without creating an installation deadlock. Registry and migration/dependency ordering continue to consume only ordinary `depends_on` / `RusToKModule::dependencies()` edges.

This closes the static/default deployment-selection source gap. Tenant-scoped lifecycle toggles and effective-policy resolution do not yet consume the package co-requisite contract, so non-default tenant activation enforcement and execution evidence remain open.

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
- the server owns a separate deployment co-requisite preflight and invokes it from startup manifest/registry validation;
- built-in installation remains on ordinary `ManifestManager::validate` and does not invoke the co-requisite preflight;
- selected co-requisites are required and version-checked without being copied into ordinary dependency ordering;
- Product uses the exact owner bootstrap services and operations;
- Product does not import foreign owner entities or query known foreign tables directly;
- the owner services remain explicitly transaction-aware;
- tenant lifecycle/effective-policy enforcement and runtime rollback evidence remain unvalidated.

## Next implementation slice

Carry the same deployment co-requisite semantics into tenant lifecycle/effective-policy selection without turning co-requisites into dependency-order edges. Enabling Product for a tenant must fail closed unless Inventory and Pricing are effectively enabled with compatible admitted owner versions; disabling either owner while Product remains effectively enabled must likewise be rejected or make Product unavailable through one canonical owner policy decision.

After tenant source enforcement exists, retain maintainer-run evidence for:

- default and non-default tenant module selections;
- clean migration ordering;
- Product create/delete rollback across all three owners;
- no silent post-commit partial state;
- no ordinary cyclic dependency;
- no Product ownership of Inventory or Pricing persistence.

Host-injected transaction participant ports remain a future alternative if Product lifecycle writes need a remote/distributed collaboration profile; they are not required to describe the current embedded atomic contract.

## Evidence

Source evidence is stored at:

`crates/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json`

No tests, Cargo commands, formatting, verifier execution, workflow checks, tenant activation, non-default selection, or transaction rollback execution evidence are claimed by this source wave.
