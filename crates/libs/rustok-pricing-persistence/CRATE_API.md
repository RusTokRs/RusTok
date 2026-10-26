# rustok-pricing-persistence / CRATE_API

## Public Modules
- `entities`: SeaORM entity definitions for `price`, `price_list`, and `price_list_translation`.

## Platform Capability Bindings
- Pricing-owned persistence library supporting `rustok-pricing` and transactional bootstrap from `rustok-product`.

## Domain Collaborations
- Holds SeaORM entity models for prices, price lists, and multilingual price list translations.
- Provides `BootstrapService` for transactional initial price creation during product and variant lifecycle mutations.
- Re-exported by `rustok-pricing` as its canonical data persistence layer.

## Minimum Contract Set

### Input DTOs/Commands
- `InitialPrice`: variant ID, optional channel ID, optional channel slug, currency code (3 uppercase ASCII letters), amount, compare-at amount.
- `BootstrapService::create_initial_prices_in_tx`: transactional creation of variant price records.
- `BootstrapService::delete_prices_for_variants_in_tx`: transactional removal of prices when variants are deleted.

### Domain Invariants
- Currency code must be exactly 3 uppercase ASCII letters (`[A-Z]{3}`).
- Amounts must be non-negative (`>= 0`).
- Variant ID and channel ID (if present) must not be nil UUIDs.
- Transactional coupling: price operations run within caller-provided SeaORM transaction context (`C: ConnectionTrait`).

### Events / Outbox Side Effects
- Pure persistence layer; domain events are staged and emitted by the owning `rustok-pricing` service.

### Errors / Failure Codes
- `sea_orm::DbErr::Custom`: validation failure (nil UUID, negative amount, invalid currency code).
- `sea_orm::DbErr`: underlying database query or transaction failure.
