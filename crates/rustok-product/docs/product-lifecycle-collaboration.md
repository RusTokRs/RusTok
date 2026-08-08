# Product lifecycle owner collaboration

Status: deployment selection and canonical owner effective-policy co-requisite identity source-complete, staged lifecycle recovery semantics and execution evidence pending, unvalidated.

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

## Canonical tenant effective-policy identity

The active composition still derives package co-requisites through `ManifestManager::module_policy_corequisites`, but the manifest layer is now only the trusted parser/translator. Availability semantics are owned by `rustok-modules`:

- `ModuleLifecycleDbWriter::with_corequisites` accepts the normalized `consumer -> provider -> version requirement` map separately from the definition catalog's ordinary dependencies;
- `ModuleEffectivePolicyQuery` normalizes the co-requisite input against the active catalog, rejects self-references, dependency/co-requisite overlap, unknown consumers/providers, conflicting duplicates, and malformed version requirements;
- the normalized co-requisite contract participates in both base and final `rustok.module_effective_policy.v2` revision input, so `policy_revision`, `EffectivePolicyCacheIdentity`, lifecycle current/next transitions, and the policy-transition outbox share one owner identity;
- the owner fixed point applies ordinary dependencies and co-requisites as distinct constraints. A missing effective owner produces `CoRequisiteUnavailable`; a selected but incompatible owner version produces `CoRequisiteVersionMismatch`;
- each Product decision carries `CoRequisite` facts with the admitted owner version, declared requirement, compatibility result, and final owner availability.

The server no longer post-validates a returned `ModuleEffectivePolicy`. Base, channel, maintenance, node-readiness, lifecycle, recovery, and settings adapters supply the active package co-requisite map to the same modules-owner writer and consume its canonical policy result.

## Tenant intent ordering

Canonical availability cannot also be the lifecycle ordering graph. Inventory and Pricing ordinarily depend on Product, while Product requires both as co-requisites. If ordinary `validate_module_toggle` consumed the co-requisite-constrained enabled set, Product could not become available before Inventory/Pricing and Inventory/Pricing could not be enabled before Product: tenant activation would deadlock.

The modules owner therefore keeps two explicit signals during normal lifecycle toggles:

- **effective availability** is co-requisite-aware and is used for serving decisions, cache identity, operation effective-state reporting, and current/next policy revision;
- **ordinary ordering selection** resolves the same tenant intent without co-requisites and is used only by `validate_module_toggle` for the existing dependency-order rules.

This lets a tenant stage Product intent first, then Inventory/Pricing, while Product remains unavailable until the complete owner set satisfies the canonical co-requisite policy. Teardown remains governed by the ordinary dependency topology; co-requisites never become a second ordering graph.

## Remaining staged recovery debt

The legacy lifecycle recovery contract persists `previous_effective_enabled` in the operation journal and uses effective availability for retry/compensation state matching. Once tenant intent and serving availability are intentionally distinct, a staged Product row can be selected while Product is effectively unavailable. In that state, `previous_effective_enabled` is not a sufficient predecessor for restoring the exact tenant intent after a post-hook failure.

This slice therefore does **not** claim staged lifecycle recovery source-complete. The next source task is to give retry/compensation an explicit selected-intent predecessor/current-state contract while retaining the co-requisite-aware policy revision for serving and outbox identity. That change must not reintroduce a second dependency-order graph.

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
- the active manifest supplies package co-requisite metadata to the modules owner without copying it into ordinary dependency ordering;
- canonical effective-policy revision input includes normalized co-requisites under the v2 contract;
- Product decisions expose explicit co-requisite facts and unavailable/version-mismatch denial reasons;
- lifecycle current/next revisions use the same co-requisite-aware owner query as policy reads;
- normal lifecycle validation uses a separate ordinary ordering selection so Product/Inventory/Pricing activation remains sequentially orderable;
- host-level duplicate effective-policy/toggle co-requisite validation is absent;
- Product uses the exact owner bootstrap services and operations;
- Product does not import foreign owner entities or query known foreign tables directly;
- the owner services remain explicitly transaction-aware;
- staged retry/compensation predecessor semantics and runtime rollback evidence remain unvalidated.

## Next implementation slice

Separate persisted tenant selected-intent predecessor/current state from effective availability in module operation recovery. Post-hook retry and compensation must prove the exact committed tenant intent they are recovering while continuing to publish the canonical co-requisite-aware policy revision. Do not change ordinary dependency ordering or turn co-requisites into toggle dependencies.

After that recovery source gap closes, retain maintainer-run evidence for:

- default and non-default tenant module selections;
- co-requisite-aware policy/cache revision changes;
- staged Product -> Inventory -> Pricing activation and reverse teardown;
- post-hook retry/compensation across staged intent;
- clean migration ordering;
- Product create/delete rollback across all three owners;
- no silent post-commit partial state;
- no ordinary cyclic dependency;
- no Product ownership of Inventory or Pricing persistence.

Host-injected transaction participant ports remain a future alternative if Product lifecycle writes need a remote/distributed collaboration profile; they are not required to describe the current embedded atomic contract.

## Evidence

Source evidence is stored at:

`crates/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json`

No tests, Cargo commands, formatting, verifier execution, workflow checks, tenant activation, non-default selection, policy-cache execution, staged recovery, or transaction rollback execution evidence are claimed by this source wave.
