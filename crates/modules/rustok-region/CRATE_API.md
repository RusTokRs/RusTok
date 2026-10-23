# Region Crate API Specification

## Platform Capability Bindings
- **RusToKModule Implementation**: `RegionModule` in `src/module.rs`
- **Module Slug**: `region`
- **Module Name**: `Region`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `[]`
- **Permissions**:
  - `REGIONS_CREATE`
  - `REGIONS_READ`
  - `REGIONS_UPDATE`
  - `REGIONS_DELETE`
  - `REGIONS_LIST`
  - `REGIONS_MANAGE`
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` (6 migrations including country tax policies and translation change journal).

## Domain Collaborations
- **Translation Targets**: Implements `RegionTranslationTargetProvider` for localized region presentation and translation journal synchronization.
- **Commerce Foundation**: Shares tax policy and region identity primitives.
- **Outbox**: Emits region lifecycle domain events through `rustok_outbox::TransactionalEventBus`.
- **FBA Port**: Exposes `RegionReadPort` for decoupled cross-boundary reading of regional configurations, currencies, and countries.

## Minimum Contract Set
- `RegionReadPort`: Decoupled read port for resolving regions, tax rates, and country associations.
- `RegionService`: Core domain CRUD operations on regional aggregates.
- `RegionTranslationService`: Regional translation snapshot and exact locale applying.

### Input DTOs/Commands
- `CreateRegionDto`: Region name, currency code, tax rate, tax code, list of countries.
- `UpdateRegionDto`: Partial updates to regional settings and tax rates.
- `CountryTaxPolicyInput`: Country code and country-specific tax rules.
- `RegionFilter`: Filtering parameters for listing regions.

### Domain Invariants
- **Non-Empty Name**: Region name must not be blank.
- **Valid Currency Code**: Currency code must be a 3-letter ISO code.
- **Country Uniqueness**: A country cannot be assigned to multiple active regions simultaneously (`CountryAlreadyAssigned`).
- **Tax Rate Precision**: Tax rates must be valid decimal percentages.

### Events / Outbox Side Effects
- `DomainEvent::RegionCreated { region_id }`
- `DomainEvent::RegionUpdated { region_id }`
- `DomainEvent::RegionDeleted { region_id }`

### Errors / Failure Codes
- `RegionError::RegionNotFound`: The requested region aggregate does not exist.
- `RegionError::CountryAlreadyAssigned`: Country is already attached to another regional grouping.
- `RegionError::Validation`: Input validation failed.
- `RegionError::Database`: Database storage failure.
