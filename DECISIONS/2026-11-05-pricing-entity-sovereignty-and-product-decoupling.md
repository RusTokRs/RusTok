# Pricing storage sovereignty and Product port decoupling

- Date: 2026-11-05
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-pricing, rustok-product
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md)
- Supersedes: [2026-03-25-commerce-module-split-product-pricing-inventory.md](./2026-03-25-commerce-module-split-product-pricing-inventory.md)
- Superseded by: None

## Context

In the initial commerce module decomposition, `rustok-pricing` was separated from `rustok-product`. However, because `rustok-pricing` depends on `rustok-product` to read product catalog projections and verify variant identities, a cyclic Cargo dependency prevented `rustok-product` from depending directly on `rustok-pricing`.

To work around this cycle, SeaORM entities (`price`, `price_list`, `price_list_translation`) and initial price write operations were placed in `crates/libs/rustok-pricing-persistence`. `rustok-product` depended on this support crate, invoked `PricingBootstrapService` in transactions, and queried the `prices` table directly (`price::Entity::find()`) in storefront listing queries.

This violated canonical architectural principles in `AGENTS.md` §5:
1. Module persistence entities and table ownership must remain sovereign to the owning domain module (`rustok-pricing`), not leaked into foundation/shared libraries (`crates/libs/*`).
2. A consumer must not read or query another module's private tables directly without an owner-defined port boundary.
3. Creating shadow persistence crates to circumvent Cargo dependency cycles undermines architectural integrity.

## Decision

1. **Sovereignty of Pricing Persistence**:
   - `rustok-pricing` is the sole owner of `prices`, `price_lists`, and `price_list_translations` tables and their migrations.
   - All SeaORM entities move directly into `crates/modules/rustok-pricing/src/entities/` and are kept crate-private (`pub(crate) mod entities;`).
   - `crates/libs/rustok-pricing-persistence` is eliminated.

2. **Decoupled Transactional Port (`ProductPricingPort`)**:
   - `rustok-product` defines a consumer-owned port `ProductPricingPort` in `rustok-product::ports::pricing` (alongside `InitialPrice` and `StorefrontProductListPrice`).
   - The port specifies operations required during product lifecycle:
     - `create_initial_prices_in_tx`: author initial variant prices inside product creation transactions.
     - `replace_catalog_base_prices_in_tx`: replace variant base prices inside product update transactions.
     - `delete_prices_for_variants_in_tx`: clean up prices inside variant/product deletion transactions.
     - `load_catalog_base_prices_for_variants`: load unscoped variant base prices for catalog projections.
     - `load_storefront_list_prices`: calculate and derive channel-scoped storefront minimum "price from" without exposing internal pricing models.
   - `CatalogService` accepts an optional `Arc<dyn ProductPricingPort>` via `with_pricing_port(...)`, exactly mirroring `with_media_reference_port` and `with_shipping_profile_port`.
   - When no port is attached (e.g. isolated Product tests), price authoring with empty prices is a safe no-op, catalog projections omit prices gracefully, and price operations fail safely if prices are provided without a pricing provider.

3. **Provider Implementation in `rustok-pricing`**:
   - `rustok-pricing` implements `ProductPricingPort` via `PricingProductPricingPortAdapter` in `src/integrations/product_pricing_port.rs` and provides the factory `in_process_product_pricing_port(db)`.
   - Because `rustok-pricing` already depends on `rustok-product`, this dependency direction is strictly acyclic: `product` (foundation) <- `pricing` (application).

4. **Zero Cross-Module SQL or Entity Leaks**:
   - `rustok-product` contains zero SQL queries against the `prices` table and zero imports of foreign pricing entities.

## Sources of truth and ownership

- `rustok-pricing` is the single source of truth for pricing models, price lists, tier rules, currency rounding discipline, and the `prices` table.
- `rustok-product` owns product variants, attributes, and catalog lifecycle.
- The boundary between product catalog and pricing is bridged exclusively via `ProductPricingPort`.

## Invariants

- `rustok-pricing` owns its database schema, migrations, and entities.
- `rustok-product` does not query or mutate `prices` table directly.
- The Cargo graph is strictly acyclic: `rustok-pricing -> rustok-product`.
- Product variant creation/deletion and pricing operations participate in atomic transactional boundaries when wired with `ProductPricingPort`.
- Dual database support (PostgreSQL and SQLite) is fully preserved.

## Non-goals

- `rustok-product` does not manage price resolution, customer group price overrides, or volume tiers.
- `rustok-pricing` does not manage catalog taxon hierarchies or stock reservations (owned by Inventory).

## Data, transaction, and concurrency boundary

In native composition sharing a database connection, `ProductPricingPort` executes operations directly against caller-provided `&DatabaseTransaction`. If price insertion fails or invariant validation fails, the entire transaction (including product mutation) rolls back. When prices are deleted as part of variant removal, cascades and deletions execute inside the same transaction.

## Context dimensions

- Tenant: All pricing operations are strictly scoped to `tenant_id`.
- Currency: ISO-4217 3-letter currency code, normalized and validated.
- Channel: Storefront price calculations accept `channel_id` for scoped price resolution.
- Locale: Non-applicable to raw prices; price list names/descriptions utilize translation structures.

## Events and projections

Pricing mutations emit `PriceCreated`, `PriceUpdated`, `PriceDeleted` domain events through the transactional outbox. Storefront search projection drivers subscribe to price events to update cached price facets.

## Failure semantics

If a product operation supplies variant prices but no `ProductPricingPort` is attached, the operation fails closed with `PricingProviderRequired`. If pricing operations fail during a transaction, the error propagates and the transaction rolls back.

## Migration and cutover

1. Migrate entities from `crates/libs/rustok-pricing-persistence` into `crates/modules/rustok-pricing/src/entities/`.
2. Introduce `ProductPricingPort` in `rustok-product`.
3. Implement `PricingProductPricingPortAdapter` in `rustok-pricing`.
4. Delete `crates/libs/rustok-pricing-persistence` and remove all Cargo dependencies on it.
5. Update server composition root to wire `in_process_product_pricing_port(db)`.

## Alternatives considered

- Retaining `crates/libs/rustok-pricing-persistence`: rejected, creates an architectural anti-pattern where domain entities live in generic libraries to bypass Cargo cycles.
- Direct raw SQL queries from Product to prices table: rejected, completely violates module isolation and schema autonomy.

## Verification

- `cargo check -p rustok-product` and `cargo test -p rustok-product --lib` pass.
- `cargo check -p rustok-pricing` and `cargo test -p rustok-pricing --lib` pass.
- `node scripts/verify/verify-module-source-layout.mjs --module pricing` passes.
- `python scripts/architecture_dependency_guard.py` passes with 0 violations.

## Consequences

- **Positive:**
  - Complete elimination of shadow persistence crate `rustok-pricing-persistence`.
  - True table sovereignty and encapsulation for `rustok-pricing`.
  - Clean, acyclic dependency graph.
- **Negative:**
  - Product commands requiring initial price setting depend on runtime injection of `ProductPricingPort`.
