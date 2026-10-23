# rustok-pricing / CRATE_API

## Public Modules
- `entities`: SeaORM entity re-exports for prices and price lists.
- `migrations`: SeaORM migration definitions for pricing schemas.
- `module`: Core `PricingModule` definition and platform registration.
- `ports`: Pricing read and write port traits and request snapshots.
- `services`: Domain services for pricing calculation, price lists, and translations.

## Platform Capability Bindings
- `PricingModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `pricing`.
- Dependencies: `product`.
- Permissions: `PRODUCTS_READ`, `PRODUCTS_UPDATE`, and `Resource::Discounts` with `Action::Create`, `Read`, `Update`, `Delete`, `List`, `Manage`.

## Domain Collaborations
- Default pricing submodule of the ecommerce family.
- Provides `PricingReadPort` for checkout repricing, storefront display, and admin product pricing projections.
- Provides `PricingWritePort` for admin variant price upserts, discount applications, and price list rules.
- Depends on `rustok-product` for variant entity links.
- Uses `rustok-pricing-persistence` for underlying pricing entities and tables.
- Implements `PriceListTranslationTargetProvider` for localized price list titles/descriptions.

## Minimum Contract Set

### Input DTOs/Commands
- `ResolveProductPriceRequest`: variant_id, currency_code (3-letter ASCII ISO), optional region_id, optional quantity, optional channel_id, optional price_list_id.
- `UpsertVariantPriceRequest`: variant_id, currency_code, price, compare_at_price, channel_id, price_list_id, min_quantity.
- `ApplyVariantDiscountRequest`: variant_id, currency_code, discount_percent, channel_id, price_list_id.
- `SetPriceListPercentageRuleRequest`: price_list_id, percentage_rule.
- `SetPriceListScopeRequest`: price_list_id, channel_id, customer_group_id.

### Domain Invariants
- Float arithmetic is strictly forbidden (`#![deny(clippy::float_arithmetic)]`); all currency and money math uses `RustDecimal`.
- Currency validation: currency codes must be exactly 3 uppercase ASCII letters.
- Quantity bounds: quantity in price resolution or tiers must be positive (`> 0`).
- Price list rules: inactive, draft, expired, or future lists are rejected without fallback writes.
- Channel scoping: price lists and base prices support optional channel scoping without pricing owning channel identity.

### Events / Outbox Side Effects
- Outbox operation receipts guarantee durable idempotency on `PricingWritePort` operations.
- Emits price change events across transactional outbox.

### Errors / Failure Codes
- `PricingPublicError::NotFound`: variant, price, or price list not found.
- `PricingPublicError::Validation`: invalid currency code, negative price, or invalid percentage.
- `PricingPublicError::Conflict`: price tier or window conflict.
- `PricingPublicError::InactivePriceList`: attempt to resolve or apply against an inactive list.
- `PricingPublicError::Database`: underlying persistence failure.
