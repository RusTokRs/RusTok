# Crate API: rustok-commerce-foundation

## Public surface

### DTOs
- `AdjustInventoryInput`: Input payload for adjusting variant inventory with validation.
- `CreateVariantInput`: Input payload for creating product variants with pricing and inventory attributes.
- `PriceInput`: Pricing configuration payload for variants with currency code and non-negative constraints.
- `PriceResponse`: Scoped price projection response with discount calculation helpers.
- `VariantResponse`: Full variant data representation with stock and currency lookup helpers.
- `VariantTranslationResponse`: Localized variant title and options.

### Error Handling
- `CommerceError`: Canonical commerce domain error enumeration:
  - `Database(DbErr)`: Database operation failure.
  - `ProductNotFound(Uuid)`: Missing product entity.
  - `VariantNotFound(Uuid)`: Missing product variant entity.
  - `DuplicateHandle { handle, locale }`: Conflicting product handle in locale.
  - `DuplicateSku(String)`: Conflicting variant SKU.
  - `InvalidPrice(String)`: Invalid pricing value.
  - `InsufficientInventory { requested, available }`: Requested stock exceeds available inventory.
  - `InvalidOptionCombination`: Variant option combination is invalid.
  - `Validation(String)`: Validation rule failure.
  - `ShippingProfileNotFound(Uuid)`: Missing shipping profile entity.
  - `DuplicateShippingProfileSlug(String)`: Conflicting shipping profile slug.
  - `NoVariants`: Product must contain at least one variant.
  - `CannotDeletePublished`: Deletion of published product forbidden.
  - `Rich(Box<RichError>)`: Wrapped rich platform error.
  - `Core(CoreError)`: Wrapped core platform error.
- `CommerceResult<T>`: Result alias parameterized by `CommerceError`.

### Search
- `MAX_SEARCH_QUERY_LEN`: Upper limit constant on search query length (255 characters).
- `product_translation_title_and_locale_search_condition`: SeaORM condition builder for title and locale search across SQLite and PostgreSQL backends.
- `product_translation_title_search_condition`: SeaORM condition builder for title search across SQLite and PostgreSQL backends.

### Entities
- `inventory_item`: `InventoryItem` SeaORM entity mapping variant-level inventory metadata.
- `inventory_level`: `InventoryLevel` SeaORM entity with `available_quantity()`, `is_low_stock()`, and `can_reserve()`.
- `product`: `Product` SeaORM entity with `ProductStatus` lifecycle graph and `is_published()`.
- `product_image`: `ProductImage` SeaORM entity mapping product media positions.
- `product_variant`: `ProductVariant` SeaORM entity with `is_in_stock()` and `allows_backorders()`.
- `reservation_item`: `ReservationItem` SeaORM entity with `is_active()` status helper.
- `shipping_profile`: `ShippingProfile` SeaORM entity with `is_usable()`.
- `shipping_profile_translation`: Localized shipping profile metadata entity.
- `stock_location`: `StockLocation` SeaORM entity with `is_active()` status helper.
- `stock_location_translation`: Localized stock location metadata entity.

## Platform Capability Bindings
- Foundation shared library for commerce modules across RusToK platform.
- Integration with SeaORM, serde, and utoipa OpenAPI schema generation.
- Two-way mapping between `CommerceError`, `RichError`, and `rustok_core::error::Error`.

## Domain Collaborations
- Shared by `rustok-product`, `rustok-pricing`, `rustok-inventory`, and `rustok-commerce`.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateVariantInput`: Variant creation command with validation rules (`validate_invariants`).
- `AdjustInventoryInput`: Variant stock delta command (`validate_invariants`).
- `PriceInput`: Currency-denominated pricing payload with ISO 4217 validation (`validate_invariants`).

### Domain Invariants
- Money amounts and price values must validate non-negativity.
- Currency codes must adhere to 3-character ISO 4217 alphabetic standards.
- Inventory policies must be valid recognized tokens (`deny`, `continue`).
- Available inventory calculated via saturating subtraction to prevent integer underflow.
- Search queries bounded to `MAX_SEARCH_QUERY_LEN` (255) with whitespace trimming and multi-backend support.

### Events / Outbox Side Effects
- DTOs and entities used in constructing domain events and outbox mutation payloads in downstream commerce crates.

### Errors / Failure Codes
- `CommerceError::ProductNotFound`: Product ID not found in tenant catalog.
- `CommerceError::VariantNotFound`: Variant ID not found in tenant catalog.
- `CommerceError::DuplicateHandle`: Handle conflict within tenant locale.
- `CommerceError::DuplicateSku`: SKU conflict within tenant catalog.
- `CommerceError::InvalidPrice`: Negative or malformed price values.
- `CommerceError::InsufficientInventory`: Out-of-stock reservation attempts.
- `CommerceError::Validation`: Input invariant validation failures.
- `CommerceError::Database`: SeaORM database failures.
- `CommerceError::ShippingProfileNotFound`: Shipping profile ID not found.
- `CommerceError::DuplicateShippingProfileSlug`: Shipping profile slug conflict.
- `CommerceError::NoVariants`: Variant collection empty on product creation.
- `CommerceError::CannotDeletePublished`: Deleting active product forbidden.
