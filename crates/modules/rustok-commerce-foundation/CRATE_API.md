# Crate API: rustok-commerce-foundation

## Public surface

### DTOs
- `AdjustInventoryInput`: Input payload for adjusting variant inventory.
- `CreateVariantInput`: Input payload for creating product variants with pricing and inventory attributes.
- `PriceInput`: Pricing configuration payload for variants.
- `PriceResponse`: Scoped price projection response.
- `VariantResponse`: Full variant data representation.
- `VariantTranslationResponse`: Localized variant title and options.

### Error Handling
- `CommerceError`: Canonical commerce domain error enumeration.
- `CommerceResult`: Result alias parameterized by `CommerceError`.

### Search
- `MAX_SEARCH_QUERY_LEN`: Upper limit constant on search query length.
- `product_translation_title_and_locale_search_condition`: SeaORM condition builder for title and locale search.
- `product_translation_title_search_condition`: SeaORM condition builder for title search.

### Entities
- Shared SeaORM entity definitions for commerce domains.

## Platform Capability Bindings
- Foundation shared library for commerce modules across RusToK platform.
- Integration with SeaORM, serde, and utoipa OpenAPI schema generation.

## Domain Collaborations
- Shared by `rustok-product`, `rustok-pricing`, `rustok-inventory`, and `rustok-commerce`.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateVariantInput`: Variant creation command with validation rules.
- `AdjustInventoryInput`: Variant stock delta command.
- `PriceInput`: Currency-denominated pricing payload.

### Domain Invariants
- Money amounts and price values must validate non-negativity.
- Currency codes must adhere to 3-character ISO standards.
- Inventory policies must be valid recognized tokens (`deny`, `continue`).

### Events / Outbox Side Effects
- DTOs and entities used in constructing domain events and outbox mutation payloads.

### Errors / Failure Codes
- `CommerceError::NotFound`: Resource missing in catalog.
- `CommerceError::Validation`: Input validation failures.
- `CommerceError::Database`: Persistence layer failures.

