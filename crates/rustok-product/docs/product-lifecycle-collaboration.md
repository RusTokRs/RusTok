# Product lifecycle owner collaboration

Status: static and tenant host co-requisite guards source-complete, canonical policy-revision integration pending, unvalidated.

## Problem

Product creation persists Product-owned variants and also accepts initial inventory and price input. The first Inventory and Pricing rows must commit or roll back with the Product write. Current source performs that collaboration through owner-owned, transaction-aware bootstrap services.

The ordinary module graph cannot represent the collaboration by adding reverse Product dependencies:

- Inventory depends on Product;
- Pricing depends on Product;
- making Product ordinarily depend on either owner would create a dependency-order cycle.

The default ecommerce activation enables Product, Pricing, and Inventory together. Source must reject unsafe effective selections without creating a second tenant enable/disable ordering graph.

## Selected source contract

Product declares Inventory and Pricing in `rustok-module.toml` under `[co_requisites]` with explicit version requirements. A co-requisite is an availability/selection constraint, not an ordinary dependency edge:

- it says the owner implementation must be available whenever Product lifecycle writes are available;
- it must not participate in dependency or migration ordering;
- it must not be copied into `ProductModule::dependencies()`;
- it therefore does not create `Product -> Inventory -> Product` or `Product -> Pricing -> Product` cycles.

The static server control plane consumes this declaration through `ManifestManager::validate_deployment_selection`. Startup `validate_registry_vs_manifest` first runs the existing ordinary manifest validation, then the deployment co-requisite preflight, then the ordinary static-registry comparison. The preflight reads selected package `[co_requisites]` and rejects a default-enabled Product when Inventory or Pricing is not also selected, when a declared requirement is malformed, or when the selected owner version is missing, invalid, or incompatible.

Deployment co-requisite validation remains separate from `ManifestManager::validate`. Built-in installation and ordinary topology validation therefore remain able to add Product, Inventory, and Pricing sequentially without creating an installation deadlock. Registry and migration/dependency ordering continue to consume only ordinary `depends_on` / `RusToKModule::dependencies()` edges.

## Tenant lifecycle and effective-policy guard

The server now derives the same package co-requisite map for the active composition through `ManifestManager::module_policy_corequisites` and applies two fail-closed host guards around the existing modules-owner policy:

- `ModuleLifecycleService` resolves the current revisioned effective policy before a tenant override write. A Product enable intent verifies that admitted Inventory and Pricing definitions exist and satisfy the declared version requirements, but it does **not** require those owners to be already enabled. This is deliberate: Inventory/Pricing ordinarily depend on Product, so requiring either side to be effectively enabled first would create an enable deadlock. Tenant intent can therefore be staged Product-first, then Inventory/Pricing; effective availability remains fail-closed until the whole owner set is active. Disable intent likewise remains governed by the ordinary dependency topology so a tenant can tear the group down in the reverse order without a second deadlock. Compensation uses the same contract check when it would re-enable a consumer; post-hook retry does not change state and needs no second selection check.
- `EffectiveModulePolicyService` validates every externally returned owner policy snapshot, including channel, maintenance, and node-readiness contexts. If Product remains enabled while an Inventory/Pricing co-requisite is unavailable or version-incompatible, the adapter returns a policy error instead of exposing an unsafe availability result.

These guards consume `ModuleEffectivePolicy` facts and enabled state; they do not mutate `ModuleDefinition.dependencies`, `validate_module_toggle`, registry comparison, or migration ordering.

This source slice deliberately does **not** claim canonical owner-policy integration yet. The package co-requisite map is not part of `ModuleEffectivePolicy.policy_revision`, and the host guard does not rewrite the owner decision with a dedicated co-requisite denial reason. That revision/cache identity gap remains the next source task before the overall dependency contract can be considered resolved.

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
- the active manifest exposes package co-requisite metadata to tenant policy guards without copying it into ordinary dependency ordering;
- Product enable intent validates admitted/version-compatible co-requisite definitions without requiring an impossible pre-enabled owner order;
- tenant enable/disable remains orderable through the ordinary dependency topology;
- externally returned effective-policy snapshots fail closed when an enabled consumer loses a co-requisite;
- Product uses the exact owner bootstrap services and operations;
- Product does not import foreign owner entities or query known foreign tables directly;
- the owner services remain explicitly transaction-aware;
- canonical policy-revision identity, runtime selection, and rollback execution evidence remain unvalidated.

## Next implementation slice

Move the co-requisite availability contract into the canonical modules-owner effective-policy identity/explanation surface without turning it into dependency ordering. The co-requisite contract must contribute to policy revision/cache identity, and an unavailable owner should produce an explicit owner decision for Product rather than only a host-level rejection.

After that source identity gap closes, retain maintainer-run evidence for:

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

No tests, Cargo commands, formatting, verifier execution, workflow checks, tenant activation, non-default selection, policy-cache execution, or transaction rollback execution evidence are claimed by this source wave.
