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
- the normalized co-requisite contract participates in both base and final `rustok.module_effective_policy.v3` revision input, so `policy_revision`, `EffectivePolicyCacheIdentity`, lifecycle current/next transitions, and the policy-transition outbox share one owner identity;
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

- products, stable Variants, unified attribute/axis assignments, translations, images, tags, and Product events;
- Product transaction admission and commit;
- collection of initial inventory and price requests from Product input.

Inventory owns:

- stock locations, inventory items, inventory levels, retained reservation/shipment/return history, and history-preserving empty-bookkeeping deletion;
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

Host-injected transaction participant ports can change local dependency wiring, but cannot
transport the same SQL transaction across processes. A future independently deployed write
profile must explicitly choose co-deployment of atomic participants or accept a different
consistency/workflow contract with receipts, failure recovery and any compensation.
Sharing a database URL or injecting a port alone does not preserve cross-process atomicity.
The existing embedded contract remains unchanged; no remote write profile is claimed.

## Evidence

Source evidence is stored at:

`crates/modules/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json`

No tests, Cargo commands, formatting, verifier execution, workflow checks, tenant activation, non-default selection, policy-cache execution, staged recovery execution, migration execution, or transaction rollback execution evidence are claimed by this source wave.

## Catalog deletion retention (2026-11-01; source only)

The existing `BootstrapService::delete_records_for_variants_in_tx` now requires the concrete
caller `DatabaseTransaction`, the current tenant and at most 256 distinct nonnil Variant IDs.
Inventory proves positive tenant Variant/item identities, any retained history reference and a
complete 4096-level plan before any effect. Every stocked/reserved/incoming counter must be
zero individually. Original locations lock before levels; empty archived locations are not
reopened. Missing/ambiguous identity cannot prove history-free state. PostgreSQL requires
READ COMMITTED; MySQL current reads and SQLite conflict handling remain backend-unverified.

Both real Product deletion callers pass the current tenant and propagate failure. Product root
deletion probes 257 child identities, refuses overflow/empty/foreign graphs and checks exact
final row counts. Publication/default-SKU/sparse-tuple rules and stable IDs remain unchanged.
Inventory cleanup means only proved-empty levels/items, never reservation/proof/receipt purge.
A later exact-effect failure rolls back prior levels, prices, Product state and event writes in
the same transaction. There is no helper commit, stock adjustment, new Inventory event or
counterfeit tombstone/revision.

Eight typed blockers feed one Product policy: invalid/limit are Validation, history/stock/changed
are Conflict, and identity/state/isolation uncertainty is InvariantViolation. HTTP maps these to
400/409/500 with fixed actionable copy; GraphQL uses the same copy/code and current correlation.
Storage remains unavailable, not a business refusal. Existing nonretryable Validation/NotFound/
Conflict receipt finalization occurs only after rollback; invariants/storage stay unknown.
Retry advice alone never establishes a terminal/no-effect outcome. Recover the original command
with its original key; do not silently turn a retained refusal into a fresh deletion.

The [Inventory contract](../../rustok-inventory/CRATE_API.md#catalog-deletion-safety-2026-11-01-source-only),
[owning ADR](../../../../DECISIONS/2026-03-25-commerce-module-split-product-pricing-inventory.md#catalog-deletion-retention-2026-11-01)
and [source packet](../../rustok-inventory/contracts/evidence/catalog-deletion-source.json)
record 31 Rust/10 Node definitions, all uncompiled/unrun, and the additive unexecuted I026 lookup
indexes. Existing migrations/receipt identities are not rewritten. This follow-up does not
retroactively change the separate lifecycle-selection wave's execution record above.

Mounted Next recovery and native/HTTP/GraphQL behavior require real execution. Leptos remaining
Product CRUD uses the generic GraphQL lifecycle gateway/redaction, not general native CRUD;
retained CRUD keys are not durable. Full actionable presentation/recovery parity and a general
retirement/disposal workflow remain incomplete. No new destructive SKU action is exposed.
Generic Inventory Index revisions/baseline/tombstones/all-writer publication and Product step 8
remain open; historical cascades or arbitrary SQL are not evidence of this business guard.

## Quantity writer consolidation (2026-11-01; source only)

`CatalogService::update_variant` now passes its concrete existing transaction and current
nonnil tenant/actor/SKU to `BootstrapService::update_initial_quantity_in_tx`. Inventory owns the
same bounded set plan used by native quantity commands, not an arbitrary first-level overwrite.
Quantity means global available units; reserved/incoming/other-location/history state is retained.
Product's existing nonnegative catalog input is preserved; native signed backorder no-op remains
read-only. No stock/item/location history is inferred or silently created by this update hook.

The selected owner default retains code=`default`, otherwise oldest active tenant location with
UUID ties. A real move requires its saved level; a missing/archived/mismatched target refuses,
without reselection after locks. Checked arithmetic and v1 event admission precede the exact
conditional stocked update. Canonical stock publication shares Product's peer metadata/attribute/
price/document transaction and later Variant publication. Any failure must roll the whole command
back through existing deferred rollback/receipt finalization; the helper never commits independently.

`QuantityWriteError` crosses the owner boundary deliberately. Eleven finite
`product.inventory_quantity_*` policies preserve kind/code/fixed safe copy through native,
HTTP and GraphQL. The uppercase public family is `PRODUCT_INVENTORY_QUANTITY_*`. Database failures
stay storage unavailable; unknown publication failures are invariants, not terminal user validation.
Only finalized deterministic Validation/NotFound/Conflict decisions can be terminal receipts.
Provider text, contradictory retry advice and copied codes with the wrong kind cannot certify an
outcome. Exact replay returns the original response/refusal without a new Inventory event.

The Product response and public-channel quantity projection now use checked wide aggregation
rather than order-dependent saturation; visibility and retained/absent stock remain different
concepts. This does not make those existing read paths an authoritative Index source.

[33 Rust/10 Node source definitions](../../rustok-inventory/contracts/evidence/quantity-writes-source.json)
remain unrun/uncompiled; real SQL/isolation/locking/concurrency/client recovery remain maintainer
gates. No client UI is changed or declared parity-complete. Installation baseline, Inventory-owned
revisions, all-writer atomic publication, tombstones and Distribution→generic Index stay open.
