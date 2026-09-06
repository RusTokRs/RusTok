# Product lifecycle owner collaboration

Status: deployment selection, canonical owner co-requisite policy identity, and staged lifecycle recovery source-complete; execution evidence pending, unvalidated.

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

The active composition derives package co-requisites through `ManifestManager::module_policy_corequisites`, but the manifest layer is only the trusted parser/translator. Availability semantics are owned by `rustok-modules`:

- `ModuleLifecycleDbWriter::with_corequisites` accepts the normalized `consumer -> provider -> version requirement` map separately from the definition catalog's ordinary dependencies;
- `ModuleEffectivePolicyQuery` normalizes the co-requisite input against the active catalog, rejects self-references, dependency/co-requisite overlap, unknown consumers/providers, conflicting duplicates, and malformed version requirements;
- the normalized co-requisite contract participates in both base and final `rustok.module_effective_policy.v2` revision input, so `policy_revision`, `EffectivePolicyCacheIdentity`, lifecycle current/next transitions, and the policy-transition outbox share one owner identity;
- the owner fixed point applies ordinary dependencies and co-requisites as distinct constraints. A missing effective owner produces `CoRequisiteUnavailable`; a selected but incompatible owner version produces `CoRequisiteVersionMismatch`;
- each Product decision carries `CoRequisite` facts with the admitted owner version, declared requirement, compatibility result, and final owner availability.

The server does not post-validate a returned `ModuleEffectivePolicy`. Base, channel, maintenance, node-readiness, lifecycle, recovery, and settings adapters supply the active package co-requisite map to the same modules-owner writer and consume its canonical policy result.

## Tenant intent ordering

Canonical availability cannot also be the lifecycle ordering graph. Inventory and Pricing ordinarily depend on Product, while Product requires both as co-requisites. If ordinary `validate_module_toggle` consumed the co-requisite-constrained enabled set, Product could not become available before Inventory/Pricing and Inventory/Pricing could not be enabled before Product: tenant activation would deadlock.

The modules owner therefore keeps two explicit signals during normal lifecycle toggles:

- **effective availability** is co-requisite-aware and is used for serving decisions, cache identity, historical operation availability, and current/next policy revision;
- **ordinary ordering selection** resolves the same tenant intent without co-requisites and is used only by `validate_module_toggle` for the existing dependency-order rules.

This lets a tenant stage Product intent first, then Inventory/Pricing, while Product remains unavailable until the complete owner set satisfies the canonical co-requisite policy. Teardown remains governed by the ordinary dependency topology; co-requisites never become a second ordering graph.

## Exact staged lifecycle recovery

Recovery now treats tenant selection and serving availability as separate owner facts. The exact explicit tenant override has three states:

- `None` — no `tenant_modules` row; selection inherits platform/default policy;
- `Some(true)` — explicit enabled override;
- `Some(false)` — explicit disabled override.

A plain boolean predecessor is insufficient because `None` and `Some(false)` have different future behavior when defaults change. New lifecycle operations therefore persist an immutable `module_operation_override_states` side record keyed by `module_operations.id`. Its row presence proves that exact selected-intent recovery evidence was recorded; nullable predecessor/target booleans can then represent inherited state without conflating it with legacy operations.

The recovery contract is fail-closed:

- legacy operations without that side record report no recorded selected-intent state and cannot be retried/compensated by guessing from `previous_effective_enabled`;
- post-hook retry proves the current explicit override still equals the operation's recorded requested override, then repeats only the post-hook. It does not require Product to be effectively available, so staged Product intent remains recoverable before Inventory/Pricing make the full set available;
- a retry attempt copies the original selected-intent predecessor/target into its own journal recovery state, preserving later compensation semantics if the retried hook fails again;
- compensation uses the inverse lifecycle hook/ordinary-order direction from the original command, but its persistence target is the exact recorded predecessor;
- when that predecessor is `None`, compensation removes the explicit `tenant_modules` row instead of manufacturing `enabled=false`;
- current/next policy revision for compensation is still computed from the exact resulting override set through the canonical co-requisite-aware owner policy, so serving/cache/outbox identity remains one source of truth.

`previous_effective_enabled` remains in the journal as historical availability evidence. It is deliberately not used as the selected-intent predecessor.

The recovery side table is introduced by a new append-only platform migration. Existing migration files and the published migration prefix are not rewritten.

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
- static deployment selection rejects incomplete/incompatible co-requisite bundles without changing ordinary install/migration ordering;
- canonical effective-policy revision input includes normalized co-requisites under the v2 contract;
- Product decisions expose explicit co-requisite facts and unavailable/version-mismatch denial reasons;
- lifecycle current/next revisions use the same co-requisite-aware owner query as policy reads;
- normal lifecycle validation uses a separate ordinary ordering selection so Product/Inventory/Pricing activation remains sequentially orderable;
- lifecycle operations persist exact nullable predecessor/target override state in a dedicated recovery side table;
- side-table row presence distinguishes inherited `None` from legacy operations with unknown predecessor state;
- post-hook retry compares exact current override state instead of effective availability and retains the original predecessor/target for the retry attempt;
- compensation runs the inverse lifecycle direction but restores the exact recorded override predecessor, including row deletion for inherited state;
- `previous_effective_enabled` is not used as the compensation target;
- the recovery migration extends the append-only migration tail;
- Product uses the exact owner bootstrap services and operations and does not take ownership of Inventory/Pricing persistence.

## Next implementation slice

The Product dependency/collaboration contract is source-resolved. The remaining gate is maintainer-run execution and retained evidence, not another ownership/topology redesign.

Retain execution evidence for:

- default and non-default tenant module selections;
- co-requisite-aware policy/cache revision changes;
- staged Product -> Inventory -> Pricing activation and reverse teardown;
- post-hook retry while Product intent is selected but Product availability is still false;
- compensation restoring explicit true, explicit false, and inherited/no-row predecessors;
- fail-closed behavior for a legacy operation without selected-intent recovery state;
- clean append-only migration ordering;
- Product create/delete rollback across all three owners;
- no silent post-commit partial state;
- no ordinary cyclic dependency;
- no Product ownership of Inventory or Pricing persistence.

Host-injected transaction participant ports remain a future alternative if Product lifecycle writes need a remote/distributed collaboration profile; they are not required to describe the current embedded atomic contract.

## Evidence

Source evidence is stored at:

`crates/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json`

No tests, Cargo commands, formatting, verifier execution, workflow checks, tenant activation, non-default selection, policy-cache execution, staged recovery execution, migration execution, or transaction rollback execution evidence are claimed by this source wave.
