# Product Bundles Crate API Specification

## Public Modules
- `dto`: Input and output data transfer objects (`CreateBundleInput`, `BundleItemInput`, `BundleDto`).
- `entities`: SeaORM entity definitions for bundles, items, and translations.
- `error`: Strongly typed domain error variants (`BundleError`, `BundleResult`).
- `migrations`: SeaORM migration chain for bundle schemas.
- `module`: Lifecycle descriptor `ProductBundlesModule` implementing `RusToKModule`.
- `ports`: Owner-defined cross-boundary port `BundlePort`.
- `services`: Domain service layer (`BundleService`) for bundle lifecycle and items.

## Platform Capability Bindings
- **RusToKModule Implementation**: `ProductBundlesModule` in `src/module.rs`
- **Module Slug**: `product_bundles`
- **Module Name**: `Product Bundles`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["product"]`
- **Permissions**:
  - `Permission::PRODUCTS_READ`
  - `Permission::PRODUCTS_UPDATE`
- **Runtime Extensions**: None.
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` (bundle tables, item relationships, translation locales, and composite foreign keys).

## Domain Collaborations
- **Product**: Extends `rustok-product` with bundle configurations, kits, item groupings, and package pricing.
- **Outbox**: Emits bundle modification events and transactional mutation receipts via `rustok-outbox`.
- **Commerce**: Ingress points for cart, checkout, and admin bundle management.

## Minimum Contract Set
- `BundleService`: Sovereign domain service managing bundle creation, item additions, updates, and removals.
- `BundlePort`: Inter-module contract for resolving and validating bundle compositions.
- `scoped_caller_idempotency_key`: Stable SHA-256 digest derivation for caller idempotency keys across native and GraphQL transports (`BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE`).

### Input DTOs/Commands
- `CreateBundleInput`: Target product ID, bundle type, status, optional discount rules.
- `UpdateBundleInput`: Bundle status, pricing overrides, item selections.
- `AddBundleItemInput`: Target bundle ID, product variant ID, quantity, sort order.
- `UpdateBundleItemInput`: Quantity, sort order, optional pricing adjustment.

### Domain Invariants
- **Strict Positive Quantities**: Item quantities in bundle compositions must be strictly positive (`quantity > 0`).
- **Composite Tenant Integrity**: All bundle parent and child items must belong strictly to the same tenant.
- **Idempotency Digest Stability**: The caller idempotency namespace formula (`commerce-graphql-bundle:<op>:<digest>`) is durable and must never change without a backward-compatible migration window.
- **Decimal Non-Negativity**: Bundle discount adjustments and price overrides must remain non-negative.

### Events / Outbox Side Effects
- Bundle creations, item mutations, and deletions stage mutation receipts and outbox events in the same database transaction before commit.

### Errors / Failure Codes
- `BundleError::NotFound`: Target bundle or child item not found.
- `BundleError::Validation`: Non-positive quantities, invalid discount formats, or NUL bytes in inputs.
- `BundleError::Conflict`: Duplicate unique constraint or idempotency receipt race.
- `BundleError::Database`: Underlying storage failure.
