# Inventory and Taxonomy storage sovereignty and Product port decoupling

- Date: 2026-11-11
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-product, rustok-inventory, rustok-taxonomy
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md), [2026-11-05-pricing-entity-sovereignty-and-product-decoupling.md](./2026-11-05-pricing-entity-sovereignty-and-product-decoupling.md)
- Supersedes: None
- Superseded by: None

## Context

Following the successful decoupling of `rustok-pricing` from `rustok-product` (ADR `2026-11-05-pricing-entity-sovereignty-and-product-decoupling.md`), an architectural isolation audit of `rustok-product` revealed that Product remains tightly coupled to `rustok-inventory` and `rustok-taxonomy`:

1. **Direct Inventory SQL & Service Coupling**:
   - `projection.rs` directly invokes `rustok_inventory::BootstrapService::load_available_quantities(conn, &variant_ids)` during storefront and catalog projection assembly.
   - `helpers.rs` and `storefront_http_read_port.rs` import public channel visibility functions directly from `rustok-inventory`.
   - `inventory.rs` places `EmbeddedInventoryBootstrapAdapter` inside `rustok-product`, invoking `rustok_inventory::BootstrapService` inside write transactions.
   - `commands.rs` and `error.rs` import `rustok_inventory::CatalogDeletionBlocker` and `QuantityWriteBlocker`.
   - `rustok-product` declares a mandatory Cargo dependency on `rustok-inventory`.

2. **Direct Taxonomy SQL & Transaction Coupling**:
   - `tags.rs` directly calls `TaxonomyService::new(self.db.clone()).resolve_term_names(...)`, querying foreign taxonomy tables.
   - `categories.rs` directly executes `sync_created_category_to_taxonomy_in_tx(&txn, ...)` within the Product category write transaction.
   - `rustok-product` declares a mandatory Cargo dependency on `rustok-taxonomy`.

3. **Impact on Fluid Backend Architecture (FBA) & Microservice Extraction**:
   - The standalone service daemon `rustok-product-catalog-service` cannot start or execute catalog read/write operations against an isolated database schema containing only `products`, `product_variants`, and `sys_events`.
   - When queries execute, missing `inventory_levels` or `taxonomy_terms` tables cause fatal SQL failures.
   - This violates `AGENTS.md` §5 (module sovereignty, zero cross-module private table reads, no shadow/coupled dependencies).

## Decision

Mirror the established and verified `ProductPricingPort` pattern across both Inventory and Taxonomy:

### 1. Inventory Decoupling (`ProductInventoryPort`)
- Define a comprehensive `ProductInventoryPort` in `rustok-product::ports::inventory`.
- The port specifies operations required during the product catalog lifecycle:
  - `load_available_quantities`: query available stock quantities for variant projections.
  - `load_available_inventory_by_variant_for_public_channel`: channel-scoped inventory derivation.
  - `bootstrap_variants_in_tx`: author initial variant stock records within product write transactions.
  - `update_initial_quantity_in_tx`: update variant quantity in transaction.
  - `delete_records_for_variants_in_tx`: clean up stock records during variant/product deletion.
- Introduce `InventoryConnection<'a>` supporting both `&DatabaseConnection` and `&DatabaseTransaction`.
- `CatalogService` accepts an optional `Arc<dyn ProductInventoryPort>` via `with_inventory_port(...)`.
- When no inventory port is attached (e.g. standalone microservice deployment or isolated Product tests):
  - Catalog projections report zero/unmanaged stock gracefully without failing SQL queries.
  - Bootstrap with empty quantities is a safe no-op.
  - Variant deletion skips inventory table cleanup.
- Relocate the concrete adapter (`InventoryProductInventoryPortAdapter`) to `crates/modules/rustok-inventory`.
- `rustok-inventory` depends on `rustok-product` (strictly acyclic: `inventory -> product`).
- Remove `rustok-inventory` from `crates/modules/rustok-product/Cargo.toml`.

### 2. Taxonomy Decoupling (`ProductTaxonomyPort`)
- Define `ProductTaxonomyPort` in `rustok-product::ports::taxonomy`.
- The port specifies tag name resolution and category synchronization contracts:
  - `resolve_tag_names`: resolve term names for product tags by tenant and locale.
  - `sync_category_in_tx`: synchronize created/updated catalog categories into the taxonomy tree.
- `CatalogService` accepts an optional `Arc<dyn ProductTaxonomyPort>` via `with_taxonomy_port(...)`.
- When no taxonomy port is attached:
  - Tags are presented by their raw IDs/slugs without foreign table lookups.
  - Category creation persists local catalog category records without foreign table mutations.
- Relocate the concrete adapter (`TaxonomyProductTaxonomyPortAdapter`) to `crates/modules/rustok-taxonomy`.
- `rustok-taxonomy` depends on `rustok-product` (strictly acyclic: `taxonomy -> product`).
- Remove `rustok-taxonomy` from `crates/modules/rustok-product/Cargo.toml`.

### 3. Composition Root Wiring
- In `apps/server`, composition root (`commerce_provider_runtime.rs`) wires:
  - `CatalogService::new(db, event_bus).with_pricing_port(...).with_inventory_port(...).with_taxonomy_port(...)`.
- In `rustok-product-catalog-service`, the daemon starts with pure local catalog storage and optional remote port adapters if configured.

## Sources of truth and ownership

- `rustok-inventory` is the single source of truth for stock: the `inventory_*` tables, stock levels, and the rules that refuse a quantity write or a deletion.
- `rustok-taxonomy` is the single source of truth for the `taxonomy_*` tables, term names, and the taxonomy tree.
- `rustok-product` owns `products`, `product_variants`, the catalog lifecycle, and the two port contracts (`rustok-product::ports::inventory`, `rustok-product::ports::taxonomy`).
- Each provider module owns its adapter (`InventoryProductInventoryPortAdapter`, `TaxonomyProductTaxonomyPortAdapter`).
- The composition root (`apps/server`, `rustok-product-catalog-service`) owns which adapters are attached.

## Invariants

- `rustok-product` has zero direct SQL queries against `inventory_*` and `taxonomy_*` tables.
- `rustok-product/Cargo.toml` contains no dependencies on `rustok-inventory` or `rustok-taxonomy`.
- The Cargo dependency graph is strictly acyclic: `pricing -> product`, `inventory -> product`, `taxonomy -> product`.
- When run against a database containing only `products`, `product_variants`, and `sys_events`, `rustok-product` operations complete without missing-table errors.
- Monolith composition in `apps/server` preserves transactional atomicity through port connections.

## Non-goals

- Merging inventory or taxonomy models into product entities.
- Removing inventory or taxonomy capabilities from the platform monolith.

## Data, transaction, and concurrency boundary

- The `*_in_tx` operations of both ports run on the `&DatabaseTransaction` of the Product write. In the monolith the Product mutation and the Inventory or Taxonomy mutation therefore commit or roll back together.
- The read operations of `ProductInventoryPort` take an `InventoryConnection<'a>`, which is either the connection or an open transaction, so a projection assembled inside a write can read through that write's transaction.
- The provider decides whether the transaction it is given is sufficient. The port reports a refusal as a finite blocker (`ProductInventoryDeletionBlocker`, `ProductInventoryQuantityBlocker`), including unsupported isolation and a concurrent change.
- One catalog deletion is bounded to `MAX_CATALOG_DELETE_VARIANTS` (256) variants; the deletion blocker `LimitExceeded` reports a larger request.

## Context dimensions

- Tenant: every write operation of the inventory port and the channel-scoped stock read carry `tenant_id`; `resolve_tag_names` resolves by tenant.
- Channel: `load_available_inventory_by_variant_for_public_channel` takes the public channel slug.
- Locale: `resolve_tag_names` resolves term names by locale. Stock quantities have no locale.
- Actor: the inventory write operations carry `actor_id`.

## Events and projections

- The decision text adds no event. Stock notifications stay with Inventory: the quantity blockers `EventUnrepresentable` and `PublicationUnverified` are part of the port contract.
- The storefront and catalog projections of Product read stock through `load_available_quantities` and `load_available_inventory_by_variant_for_public_channel`. Without an attached inventory port they report zero or unmanaged stock.
- Tag names in Product projections come from `resolve_tag_names`. Without an attached taxonomy port a tag is presented by its raw id or slug.

## Failure semantics

- No inventory port attached: stock reads return zero or unmanaged stock, a bootstrap with empty quantities is a no-op, and variant deletion skips inventory cleanup. None of these fails the Product operation.
- No taxonomy port attached: tag names are not resolved and category creation writes only the local catalog category.
- A port attached and the provider refuses: the refusal reaches the caller as `CommerceError::InventoryDeletionBlocked` or `CommerceError::InventoryQuantityBlocked` with the blocker. Inside a Product write the error fails the operation and the transaction is not committed.
- Not decided here: the outcome of a Product write that supplies non-zero initial quantities when no inventory port is attached. The pricing decision this one mirrors fails closed in the matching case (`PricingProviderRequired`).

## Migration and cutover

1. Define `ProductInventoryPort` and `ProductTaxonomyPort` in `rustok-product::ports`.
2. Move every Inventory and Taxonomy call in `rustok-product` behind the ports (`projection.rs`, `helpers.rs`, `storefront_http_read_port.rs`, `inventory.rs`, `commands.rs`, `error.rs`, `tags.rs`, `categories.rs`).
3. Implement the adapters in `rustok-inventory` and `rustok-taxonomy`.
4. Wire the adapters in the `apps/server` composition root.
5. Remove `rustok-inventory` and `rustok-taxonomy` from `crates/modules/rustok-product/Cargo.toml`.

The decision names no schema change and no data migration.

Implementation state when these sections were added (2026-11-11): of step 1, the `ProductInventoryPort` contract is in the tree (`crates/modules/rustok-product/src/ports/inventory.rs`). `ProductTaxonomyPort`, both adapters, `with_inventory_port` / `with_taxonomy_port` and step 5 are not: `crates/modules/rustok-product/Cargo.toml` still lists `rustok-taxonomy` and `rustok-inventory`. The Invariants and the checks in Verification describe the target and do not hold yet.

## Alternatives considered

- Keeping the direct Cargo dependencies and calls from `rustok-product` into Inventory and Taxonomy: rejected for the reasons in Context (the standalone catalog service cannot run on a Product-only schema, and Product reads tables it does not own).

No other alternative is recorded for this decision.

## Verification

- `cargo check -p rustok-product` passes without `rustok-inventory` and `rustok-taxonomy` in dependencies.
- `cargo test -p rustok-product --lib` passes with mock/unattached ports.
- `cargo check -p rustok-inventory` and `cargo check -p rustok-taxonomy` pass with port implementations.
- `cargo check -p rustok-product-catalog-service` passes.

## Consequences

- `rustok-inventory` and `rustok-taxonomy` gain a Cargo dependency on `rustok-product`; `rustok-product` loses both.
- `rustok-product-catalog-service` can run against a schema that holds only the Product tables.
- Stock, tag names, inventory cleanup at variant deletion, and the taxonomy copy of a category now depend on the composition root attaching the ports. A deployment that omits them does not fail: it shows zero or unmanaged stock and raw tag ids, leaves inventory records behind when variants are deleted, and creates categories without their taxonomy node.
