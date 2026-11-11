# Inventory and Taxonomy storage sovereignty and Product port decoupling

- Date: 2026-11-11
- Decision status: Proposed
- Implementation status: In Progress
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

## Invariants

- `rustok-product` has zero direct SQL queries against `inventory_*` and `taxonomy_*` tables.
- `rustok-product/Cargo.toml` contains no dependencies on `rustok-inventory` or `rustok-taxonomy`.
- The Cargo dependency graph is strictly acyclic: `pricing -> product`, `inventory -> product`, `taxonomy -> product`.
- When run against a database containing only `products`, `product_variants`, and `sys_events`, `rustok-product` operations complete without missing-table errors.
- Monolith composition in `apps/server` preserves transactional atomicity through port connections.

## Non-goals

- Merging inventory or taxonomy models into product entities.
- Removing inventory or taxonomy capabilities from the platform monolith.

## Verification

- `cargo check -p rustok-product` passes without `rustok-inventory` and `rustok-taxonomy` in dependencies.
- `cargo test -p rustok-product --lib` passes with mock/unattached ports.
- `cargo check -p rustok-inventory` and `cargo check -p rustok-taxonomy` pass with port implementations.
- `cargo check -p rustok-product-catalog-service` passes.
