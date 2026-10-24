# Product Crate API Specification

## Public surface

## Public Modules
- `dto`: Product, variant, translation, and catalog data transfer objects.
- `entities`: SeaORM entity models for products, variants, and translations.
- `error`: Strongly typed commerce error variants (`CommerceError`, `CommerceResult`).
- `fulfillment`: Product shipping and fulfillment requirement definitions.
- `migrations`: SeaORM database migrations for product schema and variant definitions.
- `module`: Lifecycle descriptor `ProductModule` implementing `RusToKModule`.
- `ports`: Owner-defined cross-boundary read ports and projection request types.
- `services`: Domain services for catalog operations, schema handling, and media references.

## Platform Capability Bindings
- **RusToKModule Implementation**: `ProductModule` in `src/module.rs`
- **Module Slug**: `product`
- **Module Name**: `Product`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["taxonomy"]`
- **Permissions**:
  - `PRODUCTS_CREATE`
  - `PRODUCTS_READ`
  - `PRODUCTS_UPDATE`
  - `PRODUCTS_DELETE`
  - `PRODUCTS_LIST`
  - `PRODUCTS_MANAGE`
- **Runtime Extensions**:
  - `ProductRuntimeSelected` typed marker registered into `ModuleRuntimeExtensions`
  - `ProductSeoTargetProvider` registered via `rustok_seo_targets::register_seo_target_provider`
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` with dependency on `taxonomy` migration sequence.

## Domain Collaborations
- **Taxonomy**: Product categorization, hierarchical taxonomy tree bindings, and storefront breadcrumbs.
- **Pricing Persistence**: Transaction-aware initial price creation via `rustok_pricing_persistence::BootstrapService::create_initial_prices_in_tx` during product variant bootstrap.
- **Inventory**: Transaction-aware initial inventory item provisioning during variant creation.
- **Flex**: Custom fields and dynamic attribute schemas attached to product aggregates.
- **Outbox & Events**: Publishes catalog lifecycle domain events through `rustok_outbox::TransactionalEventBus`.
- **SEO & Translation Targets**: Implements targets for search engine optimization and multi-locale translation engines.

## Minimum Contract Set
- `ProductCatalogCommandPort`: Core mutating command execution (create, update, publish, archive, delete products and variants).
- `ProductCatalogReadPort`: Read projections for admin and storefront consumers (`read_product_projection`, `read_variant_product_projection`, `list_published_products`).
- `ProductCatalogSchemaReadPort` & `ProductCatalogSchemaWritePort`: Custom attribute schemas, faceted search metadata, and dynamic forms.
- `ProductMediaReferencePort`: Media lifecycle tracking, release workflows, and reference audit.
- `ProductShippingProfilePort`: Fulfillment requirements and shipping classification.

### Input DTOs/Commands
- `CreateProduct`: Product name, slug/handle, category assignments, description, initial variant specifications.
- `UpdateProduct`: Partial product metadata updates, status changes, publication controls.
- `CreateVariant`: SKU, option combinations, weight, dimensions, initial pricing and inventory attributes.
- `UpdateVariant`: Variant updates including barcode, status, and attribute assignments.
- `ProductProjectionRequest`: Request for single product projection by ID, slug, or locale.
- `VariantProductProjectionRequest`: Request for product projection resolved via variant ID.
- `PublishedProductsRequest`: Storefront query for published catalog items with pagination and sorting.

### Domain Invariants
- **Non-Empty Variants**: Every product aggregate must contain at least one variant (`NoVariants`).
- **Variant Preservation**: Deleting the only remaining variant of a product is prohibited (`CannotDeleteOnlyVariant`).
- **Publication Safety**: A published product cannot be hard-deleted directly (`CannotDeletePublished`); it must first be unpublished or archived.
- **Handle Uniqueness**: Product handle/slug must be unique per locale across the tenant (`DuplicateHandle`).
- **SKU Uniqueness**: Variant SKU must be unique across the tenant catalog (`DuplicateSku`).
- **Monetary & Measurement Bounds**: Non-negative decimal validation for prices, compare-at amounts, weights, and dimensions.

### Events / Outbox Side Effects
- `DomainEvent::ProductCreated { product_id }`
- `DomainEvent::ProductUpdated { product_id }`
- `DomainEvent::ProductPublished { product_id }`
- `DomainEvent::ProductDeleted { product_id }`
- `DomainEvent::VariantCreated { product_id, variant_id }`
- `DomainEvent::VariantUpdated { product_id, variant_id }`
- `DomainEvent::VariantDeleted { product_id, variant_id }`
- `DomainEvent::ProductAttributeValuesChanged { product_id }`

### Errors / Failure Codes
- `CommerceError::ProductNotFound(Uuid)`: Target product does not exist.
- `CommerceError::DuplicateHandle { handle, locale }`: Slug conflict within the specified locale.
- `CommerceError::DuplicateSku(String)`: SKU conflict across products/variants.
- `CommerceError::Validation(String)`: Input validation failure.
- `CommerceError::Conflict(String)`: Concurrency revision conflict or invalid state transition.
- `CommerceError::NoVariants`: Attempted to save or finalize a product without variants.
- `CommerceError::VariantNotFound(Uuid)`: Target variant does not exist.
- `CommerceError::ImageNotFound(Uuid)`: Target product image does not exist.
- `CommerceError::CannotDeleteOnlyVariant`: Guard against leaving an aggregate variant-less.
- `CommerceError::CannotDeletePublished`: Guard against deleting active storefront items.
- `CommerceError::Database(DbErr)`: Underlying database error.
- `CommerceError::Core(CoreError)`: Platform core failure.
- `ProductPublicError`: Transport-safe error mapping for external consumers.
